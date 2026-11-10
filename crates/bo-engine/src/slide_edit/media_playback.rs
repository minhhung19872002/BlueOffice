//! Playback options of video and audio clips (SLIDE-MEDIA-001): trim, fades and bookmarks
//! ([MS-PPTX] §2.3.1.18 `p14:media` with §2.3.3.18 trim, §2.3.3.15 fade, §2.3.3.12 bmkLst),
//! captions ([MS-PPTX] §2.13.1.1 `p1730:tracksInfo` in the media's extension list, WebVTT Track
//! parts), and how the slide show plays the clip: the media time node (ECMA-376 Part 1, §19.5
//! `p:video` / `p:audio` → `p:cMediaNode`, CT_TLCommonMediaNodeData: `vol`, `mute`, `numSld`,
//! `showWhenStopped`) and the start effect in the timing tree (§19.5.29 cTn `presetClass`
//! `mediacall` with a §19.5 `p:cmd` call).
//!
//! Commands (`cmd`): `mediaPlayback`, `mediaBookmarkAdd`, `mediaBookmarkRemove`,
//! `mediaCaptionAdd`, `mediaCaptionRemove`, `mediaBookmarkTrigger`. Every one is a single
//! undoable transaction; unknown content of `p14:media` and of the media node is kept.

use super::build;
use super::geom::{find, place_slide};
use super::ops::{node_at, OpError, SlideOp, Tx};
use super::{selected, EditError, Outcome, SlideEditResult, SlideSel, R};
use crate::{EngineError, Result};
use bo_model::slide::{
    AnimClass, AnimEffect, AnimTrigger, Animation, Picture, Presentation, RawPart, Rel, ShapeNode,
    Timing, XmlElement, XmlNode,
};
use serde::{Deserialize, Serialize};

/// [MS-PPTX] §2.2.4 extension URI of `p14:media` in `nvPr/extLst`.
pub const MEDIA_EXT_URI: &str = "{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}";
/// [MS-PPTX] §2.2.4.1 extension URI of `p1730:tracksInfo` in `p14:media/p14:extLst`.
pub const TRACKS_EXT_URI: &str = "{3AFAAA56-56D3-431D-BCD4-E75A35582382}";
/// [MS-PPTX] §2.13 namespace of the track elements.
pub const NS_P1730: &str = "http://schemas.microsoft.com/office/powerpoint/2017/3/main";
/// [MS-PPTX] §2.1 relationship type of a Track part (content type `text/vtt`).
pub const REL_TRACK: &str = "http://schemas.microsoft.com/office/2017/04/relationships/track";
const NS_P14: &str = "http://schemas.microsoft.com/office/powerpoint/2010/main";

/// `numSld` written for "play across slides": BlueOffice's own value for "until the show ends".
pub const ACROSS_SLIDES: u32 = 999;

/// A bookmark of a clip ([MS-PPTX] §2.3.3.11 bmk).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BookmarkOut {
    /// `name`.
    pub name: String,
    /// `time`, ms from the start of the media.
    pub time_ms: f64,
}

/// A caption track of a clip ([MS-PPTX] §2.13.3.1 CT_Track).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionOut {
    /// `id` (a GUID).
    pub id: String,
    /// `label`.
    pub label: String,
    /// `lang`.
    pub lang: Option<String>,
    /// Embedded WebVTT part (bytes via `Engine::media`).
    pub part: Option<String>,
}

/// How a clip plays.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaPlayback {
    /// Time cut from the start (ms).
    pub trim_start_ms: f64,
    /// Time cut from the end (ms).
    pub trim_end_ms: f64,
    /// Fade-in duration (ms).
    pub fade_in_ms: f64,
    /// Fade-out duration (ms).
    pub fade_out_ms: f64,
    /// Bookmarks in time order.
    pub bookmarks: Vec<BookmarkOut>,
    /// `auto` (with the slide or after the previous effect), `seq` (on a click of the click
    /// sequence), `click` (a click on the clip); `None` = only by its controls.
    pub start: Option<&'static str>,
    /// Click step of the main sequence on which the clip starts (0 = with the slide).
    pub play_click: Option<usize>,
    /// Volume in percent.
    pub volume: u32,
    /// Muted.
    pub mute: bool,
    /// Slides the clip plays across (1 = this slide only).
    pub slides: u32,
    /// Loop until stopped.
    #[serde(rename = "loop")]
    pub looped: bool,
    /// Go back to the start after playing.
    pub rewind: bool,
    /// Hidden while not playing.
    pub hide_when_stopped: bool,
    /// Video: play full screen.
    pub full_screen: bool,
    /// Caption tracks.
    pub captions: Vec<CaptionOut>,
}

impl Default for MediaPlayback {
    fn default() -> Self {
        Self {
            trim_start_ms: 0.0,
            trim_end_ms: 0.0,
            fade_in_ms: 0.0,
            fade_out_ms: 0.0,
            bookmarks: Vec::new(),
            start: None,
            play_click: None,
            volume: 100,
            mute: false,
            slides: 1,
            looped: false,
            rewind: false,
            hide_when_stopped: false,
            full_screen: false,
            captions: Vec::new(),
        }
    }
}

/// Media playback commands.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum MediaCommand {
    /// Change playback options of clip `id` (default: the selected clip); absent fields stay.
    MediaPlayback {
        /// Shape id of the clip (default: the selected clip).
        #[serde(default)]
        id: Option<u32>,
        /// Time cut from the start (ms).
        #[serde(default)]
        trim_start_ms: Option<f64>,
        /// Time cut from the end (ms).
        #[serde(default)]
        trim_end_ms: Option<f64>,
        /// Fade-in duration (ms).
        #[serde(default)]
        fade_in_ms: Option<f64>,
        /// Fade-out duration (ms).
        #[serde(default)]
        fade_out_ms: Option<f64>,
        /// `auto` | `seq` | `click` | `none`.
        #[serde(default)]
        start: Option<String>,
        /// Percent, 0..=100.
        #[serde(default)]
        volume: Option<u32>,
        /// Muted.
        #[serde(default)]
        mute: Option<bool>,
        /// Slides the clip plays across (1 = this one; [`ACROSS_SLIDES`] = to the end).
        #[serde(default)]
        slides: Option<u32>,
        /// Loop until stopped.
        #[serde(default, rename = "loop")]
        looped: Option<bool>,
        /// Back to the start after playing.
        #[serde(default)]
        rewind: Option<bool>,
        /// Hidden while not playing.
        #[serde(default)]
        hide_when_stopped: Option<bool>,
        /// Video: full screen while playing.
        #[serde(default)]
        full_screen: Option<bool>,
    },
    /// Add a bookmark at `time_ms` (name: the given one, else a numbered one).
    MediaBookmarkAdd {
        /// Shape id of the clip (default: the selected clip).
        #[serde(default)]
        id: Option<u32>,
        /// Bookmark name.
        #[serde(default)]
        name: Option<String>,
        /// Time from the start of the media (ms).
        time_ms: f64,
    },
    /// Remove bookmark `name` (the effects it starts go back to a click on the clip).
    MediaBookmarkRemove {
        /// Shape id of the clip (default: the selected clip).
        #[serde(default)]
        id: Option<u32>,
        /// Bookmark name.
        name: String,
    },
    /// Add a WebVTT caption track.
    MediaCaptionAdd {
        /// Shape id of the clip (default: the selected clip).
        #[serde(default)]
        id: Option<u32>,
        /// The WebVTT file's UTF-8 text.
        text: String,
        /// Track label (default: a numbered one).
        #[serde(default)]
        label: Option<String>,
        /// Language tag of the track.
        #[serde(default)]
        lang: Option<String>,
    },
    /// Remove a caption track.
    MediaCaptionRemove {
        /// Shape id of the clip (default: the selected clip).
        #[serde(default)]
        id: Option<u32>,
        /// Track id.
        track: String,
    },
    /// Start effect `index` when clip `media` reaches bookmark `bookmark` (`None`: on a click
    /// on the clip).
    MediaBookmarkTrigger {
        /// Index of the effect in the slide's timing.
        index: usize,
        /// Shape id of the clip.
        media: u32,
        /// Bookmark name.
        #[serde(default)]
        bookmark: Option<String>,
    },
}

impl MediaCommand {
    /// The command, when `v` is one of the media playback commands.
    pub fn parse(v: &serde_json::Value) -> Option<MediaCommand> {
        let c = v.get("cmd")?.as_str()?;
        if !matches!(
            c,
            "mediaPlayback"
                | "mediaBookmarkAdd"
                | "mediaBookmarkRemove"
                | "mediaCaptionAdd"
                | "mediaCaptionRemove"
                | "mediaBookmarkTrigger"
        ) {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
}

// ---------------------------------------------------------------------------
// Times
// ---------------------------------------------------------------------------

/// [MS-PPTX] §2.3.4.6 ST_UniversalTimeOffset in ms: `TIME [UNITS]`, units `h`, `min`, `s`,
/// `ms`, `µs`, `ns`; no unit = ms (the §3 example reads `18374.0515` as 18.37 s).
pub fn parse_time_offset(s: &str) -> Option<f64> {
    let s = s.trim();
    let digits = s
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(s.len());
    let (num, unit) = s.split_at(digits);
    let v: f64 = num.parse().ok()?;
    let k = match unit {
        "" | "ms" => 1.0,
        "h" => 3_600_000.0,
        "min" => 60_000.0,
        "s" => 1000.0,
        "µs" => 0.001,
        "ns" => 0.000_001,
        _ => return None,
    };
    Some(v * k).filter(|t| t.is_finite())
}

/// A time in ms as ST_UniversalTimeOffset (ms, at most 4 decimals, no unit).
pub fn format_time_offset(ms: f64) -> String {
    let v = (ms.max(0.0) * 10_000.0).round() / 10_000.0;
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() {
        "0".into()
    } else {
        s.into()
    }
}

/// W3C WebVTT §4.1: a file starts with an optional BOM, `WEBVTT`, then a space, tab, line
/// break or the end.
pub fn is_webvtt(text: &str) -> bool {
    let t = text.strip_prefix('\u{feff}').unwrap_or(text);
    t.strip_prefix("WEBVTT")
        .is_some_and(|r| r.is_empty() || r.starts_with([' ', '\t', '\n', '\r']))
}

// ---------------------------------------------------------------------------
// XML helpers
// ---------------------------------------------------------------------------

fn descendants<'a>(el: &'a XmlElement, out: &mut Vec<&'a XmlElement>) {
    for c in el.elements() {
        out.push(c);
        descendants(c, out);
    }
}

fn set_attr(el: &mut XmlElement, k: &str, v: Option<String>) {
    match v {
        Some(v) => match el.attrs.iter_mut().find(|(n, _)| n == k) {
            Some((_, old)) => *old = v,
            None => el.attrs.push((k.to_owned(), v)),
        },
        None => el.attrs.retain(|(n, _)| n != k),
    }
}

fn child_mut<'a>(el: &'a mut XmlElement, local: &str) -> Option<&'a mut XmlElement> {
    el.children.iter_mut().find_map(|c| match c {
        XmlNode::Element(e) if e.local() == local => Some(e),
        _ => None,
    })
}

/// The child `local` of `el`, created (as `name`) at its place in `order` when missing.
fn ensure_child<'a>(el: &'a mut XmlElement, name: &str, order: &[&str]) -> R<&'a mut XmlElement> {
    let local = name.rsplit(':').next().unwrap_or(name);
    let rank = |l: &str| order.iter().position(|o| *o == l).unwrap_or(order.len());
    if !el.elements().any(|e| e.local() == local) {
        let pos = el
            .children
            .iter()
            .position(|c| matches!(c, XmlNode::Element(e) if rank(e.local()) > rank(local)))
            .unwrap_or(el.children.len());
        el.children
            .insert(pos, XmlNode::Element(XmlElement::new(name)));
    }
    child_mut(el, local).ok_or(EditError::Refused("noMedia"))
}

fn remove_children(el: &mut XmlElement, local: &str) {
    el.children
        .retain(|c| !matches!(c, XmlNode::Element(e) if e.local() == local));
}

pub(crate) fn parse_el(xml: &str) -> R<XmlElement> {
    bo_drawingml::dom::parse(xml.as_bytes(), "/media").map_err(|_| EditError::Refused("badMedia"))
}

/// [MS-PPTX] CT_Media child order.
const MEDIA_ORDER: &[&str] = &["trim", "fade", "bmkLst", "extLst"];

/// The clip's `p14:media` (in `nvPr/extLst/ext[@uri]`).
fn p14_media(pic: &Picture) -> Option<&XmlElement> {
    pic.nv
        .nv_extra
        .iter()
        .filter(|x| x.xml.local() == "extLst")
        .flat_map(|x| x.xml.elements())
        .filter(|e| e.local() == "ext" && e.attr("uri") == Some(MEDIA_EXT_URI))
        .flat_map(XmlElement::elements)
        .find(|e| e.local() == "media")
}

fn p14_media_mut(pic: &mut Picture) -> Option<&mut XmlElement> {
    pic.nv
        .nv_extra
        .iter_mut()
        .filter(|x| x.xml.local() == "extLst")
        .flat_map(|x| x.xml.children.iter_mut())
        .filter_map(|c| match c {
            XmlNode::Element(e) if e.local() == "ext" && e.attr("uri") == Some(MEDIA_EXT_URI) => {
                Some(e)
            }
            _ => None,
        })
        .flat_map(|e| e.children.iter_mut())
        .find_map(|c| match c {
            XmlNode::Element(e) if e.local() == "media" => Some(e),
            _ => None,
        })
}

/// `a:videoFile` (true) or `a:audioFile` (false) of a clip.
fn is_video(pic: &Picture) -> Option<bool> {
    let mut all = Vec::new();
    for x in &pic.nv.nv_extra {
        all.push(&x.xml);
        descendants(&x.xml, &mut all);
    }
    all.iter().find_map(|e| match e.name.as_str() {
        "a:videoFile" => Some(true),
        "a:audioFile" => Some(false),
        _ => None,
    })
}

// ---------------------------------------------------------------------------
// Timing: media node and start effect
// ---------------------------------------------------------------------------

/// Does `el` target shape `id` (§19.5.72 spTgt)?
fn targets(el: &XmlElement, id: u32) -> bool {
    let mut all = Vec::new();
    descendants(el, &mut all);
    all.iter()
        .any(|e| e.name == "p:spTgt" && e.attr("spid").and_then(|v| v.parse().ok()) == Some(id))
}

/// Shapes `ids` were deleted: their media nodes go, and effects their bookmarks started go
/// back to the main sequence (bmkTgt MUST name an existing clip's bookmark, §2.3.3.13).
pub fn forget_clips(t: &mut Timing, ids: &[u32]) {
    t.others.retain(|o| {
        !(matches!(o.local(), "video" | "audio") && ids.iter().any(|id| targets(o, *id)))
    });
    for a in &mut t.animations {
        if a.trigger_bookmark.is_some() && a.trigger_shape.is_some_and(|s| ids.contains(&s)) {
            a.trigger_shape = None;
            a.trigger_bookmark = None;
        }
    }
}

/// Index in `others` of the media node (§19.5 `p:video` / `p:audio`) of clip `id`.
fn media_node(others: &[XmlElement], id: u32) -> Option<usize> {
    others
        .iter()
        .position(|o| matches!(o.local(), "video" | "audio") && targets(o, id))
}

/// The `p:cmd` (§19.5 CT_TLCommandBehavior) of a media call effect.
pub fn media_cmd(a: &Animation) -> Option<&str> {
    if a.class != AnimClass::Other("mediacall".into()) {
        return None;
    }
    let src = a.source.as_ref()?;
    let mut all = Vec::new();
    descendants(src, &mut all);
    all.iter()
        .find(|e| e.name == "p:cmd")
        .and_then(|e| e.attr("cmd"))
}

/// A start effect of clip `id`: a play (`playFrom`) or pause / resume (`togglePause`) call not
/// triggered by a bookmark.
fn is_start_effect(a: &Animation, id: u32) -> bool {
    a.shape_id == id
        && a.trigger_bookmark.is_none()
        && media_cmd(a).is_some_and(|c| c.starts_with("playFrom") || c == "togglePause")
}

/// The `p:par` of a media call effect: §19.5.29 cTn `presetClass="mediacall"` holding a
/// §19.5 `p:cmd type="call"` on the clip. Preset ids: BlueOffice writes 1 for play and 2 for
/// pause / resume.
fn media_call(id: u32, cmd: &str, preset: u32) -> R<XmlElement> {
    parse_el(&format!(
        concat!(
            r#"<p:par><p:cTn id="1" presetID="{preset}" presetClass="mediacall" presetSubtype="0" fill="hold" nodeType="clickEffect">"#,
            r#"<p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst>"#,
            r#"<p:cmd type="call" cmd="{cmd}"><p:cBhvr><p:cTn id="2" dur="1" fill="hold"/>"#,
            r#"<p:tgtEl><p:spTgt spid="{id}"/></p:tgtEl></p:cBhvr></p:cmd>"#,
            r#"</p:childTnLst></p:cTn></p:par>"#
        ),
        preset = preset,
        cmd = cmd,
        id = id
    ))
}

pub(crate) fn start_effect(id: u32, mode: &str) -> R<Animation> {
    let (cmd, preset, trigger, shape) = match mode {
        "auto" => ("playFrom(0.0)", 1, AnimTrigger::AfterPrevious, None),
        "seq" => ("playFrom(0.0)", 1, AnimTrigger::OnClick, None),
        "click" => ("togglePause", 2, AnimTrigger::OnClick, Some(id)),
        _ => return Err(EditError::Refused("badMediaStart")),
    };
    Ok(Animation {
        shape_id: id,
        class: AnimClass::Other("mediacall".into()),
        effect: AnimEffect::Other,
        direction: None,
        trigger,
        delay_ms: 0,
        dur_ms: 1,
        preset_id: Some(preset),
        preset_subtype: Some(0),
        repeat: None,
        trigger_shape: shape,
        trigger_bookmark: None,
        source: Some(media_call(id, cmd, preset)?),
    })
}

/// A new media node of clip `id` (§19.5 CT_TLMediaNodeVideo / CT_TLMediaNodeAudio):
/// started by its call effects (`delay="indefinite"`), full volume.
pub(crate) fn new_media_node(id: u32, video: bool) -> R<XmlElement> {
    let tag = if video { "p:video" } else { "p:audio" };
    parse_el(&format!(
        concat!(
            r#"<{tag}><p:cMediaNode vol="100000"><p:cTn id="1" fill="hold">"#,
            r#"<p:stCondLst><p:cond delay="indefinite"/></p:stCondLst></p:cTn>"#,
            r#"<p:tgtEl><p:spTgt spid="{id}"/></p:tgtEl></p:cMediaNode></{tag}>"#
        ),
        tag = tag,
        id = id
    ))
}

fn bool_attr(el: &XmlElement, k: &str) -> Option<bool> {
    el.attr(k).and_then(bo_drawingml::dom::parse_bool)
}

/// Playback options of the clip `pic` (shape `id`) on `slide`.
pub fn playback_of(pres: &Presentation, slide: usize, pic: &Picture, id: u32) -> MediaPlayback {
    let mut out = MediaPlayback::default();
    let Some(s) = pres.slides.get(slide) else {
        return out;
    };
    if let Some(m) = p14_media(pic) {
        let t = |el: Option<&XmlElement>, k: &str| {
            el.and_then(|e| e.attr(k))
                .and_then(parse_time_offset)
                .unwrap_or(0.0)
        };
        let trim = m.elements().find(|e| e.local() == "trim");
        let fade = m.elements().find(|e| e.local() == "fade");
        out.trim_start_ms = t(trim, "st");
        out.trim_end_ms = t(trim, "end");
        out.fade_in_ms = t(fade, "in");
        out.fade_out_ms = t(fade, "out");
        out.bookmarks = m
            .elements()
            .filter(|e| e.local() == "bmkLst")
            .flat_map(XmlElement::elements)
            .filter(|e| e.local() == "bmk")
            .map(|b| BookmarkOut {
                name: b.attr("name").unwrap_or_default().to_owned(),
                time_ms: b.attr("time").and_then(parse_time_offset).unwrap_or(0.0),
            })
            .collect();
        out.bookmarks
            .sort_by(|a, b| a.time_ms.total_cmp(&b.time_ms));
        let mut all = Vec::new();
        descendants(m, &mut all);
        out.captions = all
            .iter()
            .filter(|e| e.local() == "track")
            .map(|tr| {
                let rid = tr
                    .attrs
                    .iter()
                    .find(|(k, _)| k.ends_with(":link") || k.ends_with(":embed"))
                    .map(|(_, v)| v.as_str());
                let part = rid
                    .and_then(|r| s.info.rels.iter().find(|x| x.id == r))
                    .filter(|r| !r.external)
                    .map(|r| bo_opc::resolve_target(&s.info.part_name, &r.target));
                CaptionOut {
                    id: tr.attr("id").unwrap_or_default().to_owned(),
                    label: tr.attr("label").unwrap_or_default().to_owned(),
                    lang: tr.attr("lang").map(str::to_owned),
                    part,
                }
            })
            .collect();
    }
    let Some(timing) = &s.timing else {
        return out;
    };
    if let Some(node) = media_node(&timing.others, id).map(|i| &timing.others[i]) {
        if let Some(cm) = node.child("p:cMediaNode") {
            // CT_TLCommonMediaNodeData defaults: vol 50000, mute false, numSld 1,
            // showWhenStopped true.
            let vol = cm
                .attr("vol")
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(50_000);
            out.volume = (vol / 1000).min(100);
            out.mute = bool_attr(cm, "mute").unwrap_or(false);
            out.slides = cm
                .attr("numSld")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1)
                .max(1);
            out.hide_when_stopped = !bool_attr(cm, "showWhenStopped").unwrap_or(true);
            if let Some(ctn) = cm.child("p:cTn") {
                out.looped = ctn.attr("repeatCount") == Some("indefinite");
                // BlueOffice's mapping: §19.5.29 fill="remove" drops the node's state when it
                // ends, i.e. the clip goes back to its start.
                out.rewind = ctn.attr("fill") == Some("remove");
            }
        }
        out.full_screen = bool_attr(node, "fullScrn").unwrap_or(false);
    }
    let timeline = timing.timeline();
    let main = timing.animations.iter().enumerate().find(|(_, a)| {
        a.trigger_shape.is_none()
            && is_start_effect(a, id)
            && media_cmd(a).is_some_and(|c| c.starts_with("playFrom"))
    });
    if let Some((i, _)) = main {
        let click = timeline
            .iter()
            .find(|e| e.animation == i)
            .map_or(0, |e| e.click);
        out.play_click = Some(click);
        out.start = Some(if click == 0 { "auto" } else { "seq" });
    } else if timing
        .animations
        .iter()
        .any(|a| a.trigger_shape == Some(id) && is_start_effect(a, id))
    {
        out.start = Some("click");
    }
    out
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// The clip `id` (or the first selected clip): its index path and picture.
fn clip_at(pres: &Presentation, sel: &SlideSel, id: Option<u32>) -> R<(Vec<usize>, Picture, u32)> {
    let placed = place_slide(pres, sel.slide);
    let candidates: Vec<&super::geom::Placed> = match id {
        Some(i) => find(&placed, i).into_iter().collect(),
        None => selected(&placed, &sel.ids),
    };
    for p in candidates {
        if let Some(ShapeNode::Picture(pic)) = node_at(pres, sel.slide, &p.path) {
            if is_video(pic).is_some() {
                return Ok((p.path.clone(), (**pic).clone(), p.id));
            }
        }
    }
    Err(EditError::Refused("noMedia"))
}

fn put_clip(tx: &mut Tx<'_>, slide: usize, path: &[usize], pic: Picture) -> R<()> {
    tx.replace_node(slide, path, ShapeNode::Picture(Box::new(pic)))?;
    Ok(())
}

/// `p14:media` of a clip, created in `nvPr/extLst` when the clip has none (a linked clip
/// written by an older producer); `embed` is the media relationship of a new one.
fn ensure_p14_media(pic: &mut Picture) -> R<&mut XmlElement> {
    if p14_media(pic).is_none() {
        // Reuse the link of the videoFile / audioFile.
        let mut all = Vec::new();
        for x in &pic.nv.nv_extra {
            all.push(&x.xml);
            descendants(&x.xml, &mut all);
        }
        let link = all
            .iter()
            .find(|e| matches!(e.name.as_str(), "a:videoFile" | "a:audioFile"))
            .and_then(|e| e.attr("r:link"))
            .ok_or(EditError::Refused("noMedia"))?
            .to_owned();
        let ext = parse_el(&format!(
            r#"<p:ext uri="{MEDIA_EXT_URI}"><p14:media xmlns:p14="{NS_P14}" r:link="{link}"/></p:ext>"#
        ))?;
        if let Some(x) = pic
            .nv
            .nv_extra
            .iter_mut()
            .find(|x| x.xml.local() == "extLst")
        {
            x.xml.children.push(XmlNode::Element(ext));
        } else {
            let mut l = XmlElement::new("p:extLst");
            l.children.push(XmlNode::Element(ext));
            // After the media file (§19.3.1.33 nvPr: …, media, custDataLst, extLst).
            let key = pic.nv.nv_extra.iter().map(|x| x.key).max().unwrap_or(0) + 1;
            pic.nv.nv_extra.push(bo_model::slide::Extra { key, xml: l });
        }
    }
    p14_media_mut(pic).ok_or(EditError::Refused("noMedia"))
}

/// Write trim / fade of `m` ([MS-PPTX] §2.3.3.18 CT_MediaTrim, §2.3.3.15 CT_MediaFade): an
/// element with both values 0 is dropped.
fn set_pair(m: &mut XmlElement, el: &str, keys: [&str; 2], vals: [Option<f64>; 2]) -> R<()> {
    if vals.iter().all(Option::is_none) {
        return Ok(());
    }
    let cur: Vec<f64> = keys
        .iter()
        .map(|k| {
            m.elements()
                .find(|e| e.local() == el)
                .and_then(|e| e.attr(k))
                .and_then(parse_time_offset)
                .unwrap_or(0.0)
        })
        .collect();
    let next = [vals[0].unwrap_or(cur[0]), vals[1].unwrap_or(cur[1])];
    if next.iter().all(|v| *v <= 0.0) {
        remove_children(m, el);
        return Ok(());
    }
    let e = ensure_child(m, &format!("p14:{el}"), MEDIA_ORDER)?;
    for (k, v) in keys.iter().zip(next) {
        set_attr(e, k, (v > 0.0).then(|| format_time_offset(v)));
    }
    Ok(())
}

fn valid_ms(v: Option<f64>) -> R<()> {
    match v {
        Some(t) if !t.is_finite() || t < 0.0 || t > 86_400_000.0 => {
            Err(EditError::Refused("badTime"))
        }
        _ => Ok(()),
    }
}

/// Edit the timing of `slide` through `f`.
fn edit_timing(tx: &mut Tx<'_>, slide: usize, f: impl FnOnce(&mut Timing) -> R<()>) -> R<()> {
    let s = tx.pres.slides.get(slide).ok_or(OpError::NoSlide(slide))?;
    let mut t = s.timing.clone().unwrap_or_default();
    f(&mut t)?;
    tx.fields(slide, |fl| fl.timing = Some(t))?;
    Ok(())
}

/// Options of the media node.
#[derive(Default)]
struct NodeSet {
    volume: Option<u32>,
    mute: Option<bool>,
    slides: Option<u32>,
    looped: Option<bool>,
    rewind: Option<bool>,
    hide_when_stopped: Option<bool>,
    full_screen: Option<bool>,
}

fn apply_node(node: &mut XmlElement, n: &NodeSet, video: bool) {
    if video {
        if let Some(f) = n.full_screen {
            // §19.5 CT_TLMediaNodeVideo@fullScrn.
            set_attr(node, "fullScrn", f.then(|| "1".to_owned()));
        }
    }
    let Some(cm) = child_mut(node, "cMediaNode") else {
        return;
    };
    // CT_TLCommonMediaNodeData: vol (ST_PositiveFixedPercentage, 100000 = 100 %), mute,
    // numSld, showWhenStopped.
    if let Some(v) = n.volume {
        set_attr(cm, "vol", Some((v.min(100) * 1000).to_string()));
    }
    if let Some(m) = n.mute {
        set_attr(cm, "mute", m.then(|| "1".to_owned()));
    }
    if let Some(s) = n.slides {
        set_attr(cm, "numSld", (s > 1).then(|| s.to_string()));
    }
    if let Some(h) = n.hide_when_stopped {
        set_attr(cm, "showWhenStopped", h.then(|| "0".to_owned()));
    }
    if let Some(ctn) = child_mut(cm, "cTn") {
        if let Some(l) = n.looped {
            // §19.5.29 repeatCount.
            set_attr(ctn, "repeatCount", l.then(|| "indefinite".to_owned()));
        }
        if let Some(r) = n.rewind {
            set_attr(
                ctn,
                "fill",
                Some(if r { "remove" } else { "hold" }.to_owned()),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_playback(
    tx: &mut Tx<'_>,
    sel: &SlideSel,
    id: Option<u32>,
    trim: [Option<f64>; 2],
    fade: [Option<f64>; 2],
    start: Option<&str>,
    node: &NodeSet,
) -> R<()> {
    for v in trim.iter().chain(&fade) {
        valid_ms(*v)?;
    }
    if node.volume.is_some_and(|v| v > 100) {
        return Err(EditError::Refused("badVolume"));
    }
    let (path, mut pic, id) = clip_at(tx.pres, sel, id)?;
    let video = is_video(&pic).unwrap_or(true);
    if trim.iter().chain(&fade).any(Option::is_some) {
        let m = ensure_p14_media(&mut pic)?;
        set_pair(m, "trim", ["st", "end"], trim)?;
        set_pair(m, "fade", ["in", "out"], fade)?;
        put_clip(tx, sel.slide, &path, pic)?;
    }
    let start = start.map(|s| if s == "none" { "" } else { s });
    if let Some(s) = start {
        if !matches!(s, "" | "auto" | "seq" | "click") {
            return Err(EditError::Refused("badMediaStart"));
        }
    }
    let touches_node = node.volume.is_some()
        || node.mute.is_some()
        || node.slides.is_some()
        || node.looped.is_some()
        || node.rewind.is_some()
        || node.hide_when_stopped.is_some()
        || node.full_screen.is_some();
    if start.is_none() && !touches_node {
        return Ok(());
    }
    edit_timing(tx, sel.slide, |t| {
        let i = if let Some(i) = media_node(&t.others, id) {
            i
        } else {
            t.others.push(new_media_node(id, video)?);
            t.others.len() - 1
        };
        apply_node(&mut t.others[i], node, video);
        if let Some(mode) = start {
            t.animations.retain(|a| !is_start_effect(a, id));
            if !mode.is_empty() {
                let a = start_effect(id, mode)?;
                if mode == "auto" {
                    // First of the main sequence: plays with the slide.
                    t.animations.insert(0, a);
                } else {
                    t.animations.push(a);
                }
            }
        }
        Ok(())
    })
}

fn cmd_bookmark_add(
    tx: &mut Tx<'_>,
    sel: &SlideSel,
    id: Option<u32>,
    name: Option<&str>,
    time_ms: f64,
) -> R<()> {
    valid_ms(Some(time_ms))?;
    let (path, mut pic, _) = clip_at(tx.pres, sel, id)?;
    let m = ensure_p14_media(&mut pic)?;
    let list = ensure_child(m, "p14:bmkLst", MEDIA_ORDER)?;
    let names: Vec<String> = list
        .elements()
        .filter_map(|b| b.attr("name").map(str::to_owned))
        .collect();
    // §2.3.3.11: names and times are unique in the list.
    let time = format_time_offset(time_ms);
    if list.elements().any(|b| {
        b.attr("time")
            .and_then(parse_time_offset)
            .is_some_and(|t| format_time_offset(t) == time)
    }) {
        return Err(EditError::Refused("bookmarkExists"));
    }
    let name = match name.map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) if names.iter().any(|x| x == n) => {
            return Err(EditError::Refused("bookmarkExists"))
        }
        Some(n) => n.to_owned(),
        None => (1..=names.len() + 1)
            .map(|k| format!("Bookmark {k}"))
            .find(|n| !names.contains(n))
            .unwrap_or_default(),
    };
    let mut b = XmlElement::new("p14:bmk");
    b.attrs.push(("name".into(), name));
    b.attrs.push(("time".into(), time));
    // In time order.
    let at = list
        .children
        .iter()
        .position(|c| {
            matches!(c, XmlNode::Element(e)
                if e.attr("time").and_then(parse_time_offset).is_some_and(|t| t > time_ms))
        })
        .unwrap_or(list.children.len());
    list.children.insert(at, XmlNode::Element(b));
    put_clip(tx, sel.slide, &path, pic)
}

fn cmd_bookmark_remove(tx: &mut Tx<'_>, sel: &SlideSel, id: Option<u32>, name: &str) -> R<()> {
    let (path, mut pic, id) = clip_at(tx.pres, sel, id)?;
    let m = p14_media_mut(&mut pic).ok_or(EditError::Refused("noBookmark"))?;
    let list = child_mut(m, "bmkLst").ok_or(EditError::Refused("noBookmark"))?;
    let n = list.children.len();
    list.children
        .retain(|c| !matches!(c, XmlNode::Element(e) if e.attr("name") == Some(name)));
    if list.children.len() == n {
        return Err(EditError::Refused("noBookmark"));
    }
    if list.elements().next().is_none() {
        remove_children(m, "bmkLst");
    }
    put_clip(tx, sel.slide, &path, pic)?;
    // Effects started by the bookmark: started by a click on the clip instead (bmkName MUST
    // name a bookmark of the list, §2.3.3.13).
    let uses = tx
        .pres
        .slides
        .get(sel.slide)
        .and_then(|s| s.timing.as_ref())
        .is_some_and(|t| {
            t.animations
                .iter()
                .any(|a| a.trigger_shape == Some(id) && a.trigger_bookmark.as_deref() == Some(name))
        });
    if uses {
        edit_timing(tx, sel.slide, |t| {
            for a in &mut t.animations {
                if a.trigger_shape == Some(id) && a.trigger_bookmark.as_deref() == Some(name) {
                    a.trigger_bookmark = None;
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}

use crate::guid::guid_nth;

fn cmd_caption_add(
    tx: &mut Tx<'_>,
    sel: &SlideSel,
    id: Option<u32>,
    text: &str,
    label: Option<&str>,
    lang: Option<&str>,
) -> R<()> {
    if !is_webvtt(text) {
        return Err(EditError::Refused("badCaptions"));
    }
    let (path, mut pic, _) = clip_at(tx.pres, sel, id)?;
    let part = build::unique_part(tx.pres, "/ppt/media/track", "vtt");
    let s = tx
        .pres
        .slides
        .get(sel.slide)
        .ok_or(OpError::NoSlide(sel.slide))?;
    let rid = (1..=s.info.rels.len() + 1)
        .map(|n| format!("rId{n}"))
        .find(|r| s.info.rels.iter().all(|x| &x.id != r))
        .unwrap_or_default();
    let existing: Vec<String> = playback_of(tx.pres, sel.slide, &pic, 0)
        .captions
        .into_iter()
        .map(|c| c.id)
        .collect();
    let track_id = (0..=existing.len() as u64)
        .map(|n| guid_nth(&format!("{part}{text}"), n))
        .find(|g| !existing.contains(g))
        .unwrap_or_default();
    let label = label
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map_or_else(|| format!("Track {}", existing.len() + 1), str::to_owned);
    let m = ensure_p14_media(&mut pic)?;
    let list = ensure_child(m, "p14:extLst", MEDIA_ORDER)?;
    let has_ext = list
        .elements()
        .any(|e| e.local() == "ext" && e.attr("uri") == Some(TRACKS_EXT_URI));
    if !has_ext {
        list.children.push(XmlNode::Element(parse_el(&format!(
            concat!(
                r#"<p:ext uri="{uri}"><p1730:tracksInfo xmlns:p1730="{ns}" displayLoc="media">"#,
                r#"<p1730:trackLst/></p1730:tracksInfo></p:ext>"#
            ),
            uri = TRACKS_EXT_URI,
            ns = NS_P1730
        ))?));
    }
    let ext = list
        .children
        .iter_mut()
        .find_map(|c| match c {
            XmlNode::Element(e) if e.local() == "ext" && e.attr("uri") == Some(TRACKS_EXT_URI) => {
                Some(e)
            }
            _ => None,
        })
        .ok_or(EditError::Refused("noMedia"))?;
    let prefix = ext
        .elements()
        .find(|e| e.local() == "tracksInfo")
        .and_then(|e| e.name.split_once(':').map(|(p, _)| p.to_owned()))
        .unwrap_or_else(|| "p1730".into());
    let info = child_mut(ext, "tracksInfo").ok_or(EditError::Refused("noMedia"))?;
    let lst = ensure_child(info, &format!("{prefix}:trackLst"), &["trackLst"])?;
    // [MS-PPTX] §2.13.3.1 CT_Track: id, label, lang, r:embed.
    let mut tr = XmlElement::new(&format!("{prefix}:track"));
    tr.attrs.push(("id".into(), track_id));
    tr.attrs.push(("label".into(), label));
    if let Some(l) = lang.map(str::trim).filter(|l| !l.is_empty()) {
        tr.attrs.push(("lang".into(), l.to_owned()));
    }
    tr.attrs.push(("r:embed".into(), rid.clone()));
    lst.children.push(XmlNode::Element(tr));
    let at = tx.pres.parts.len();
    tx.op(SlideOp::Parts {
        index: at,
        remove: vec![],
        insert: vec![RawPart {
            name: part.clone(),
            content_type: Some("text/vtt".into()),
            data: text.as_bytes().to_vec(),
        }],
    })?;
    let target = format!("../media/{}", part.rsplit('/').next().unwrap_or_default());
    tx.fields(sel.slide, |f| {
        f.rels.push(Rel {
            id: rid,
            rel_type: REL_TRACK.into(),
            target,
            external: false,
        });
    })?;
    put_clip(tx, sel.slide, &path, pic)
}

fn cmd_caption_remove(tx: &mut Tx<'_>, sel: &SlideSel, id: Option<u32>, track: &str) -> R<()> {
    let (path, mut pic, _) = clip_at(tx.pres, sel, id)?;
    let m = p14_media_mut(&mut pic).ok_or(EditError::Refused("noCaptions"))?;
    let mut rid = None;
    remove_track(m, track, &mut rid);
    let rid = rid.ok_or(EditError::Refused("noCaptions"))?;
    // An emptied tracks extension (and extension list) goes away.
    if let Some(list) = child_mut(m, "extLst") {
        list.children.retain(|c| {
            !matches!(c, XmlNode::Element(e)
                if e.attr("uri") == Some(TRACKS_EXT_URI)
                    && !e.elements().any(|i| i.elements().any(|l| l.elements().next().is_some())))
        });
        if list.elements().next().is_none() {
            remove_children(m, "extLst");
        }
    }
    put_clip(tx, sel.slide, &path, pic)?;
    let part = tx.pres.slides.get(sel.slide).and_then(|s| {
        s.info
            .rels
            .iter()
            .find(|r| r.id == rid && !r.external)
            .map(|r| bo_opc::resolve_target(&s.info.part_name, &r.target))
    });
    tx.fields(sel.slide, |f| f.rels.retain(|r| r.id != rid))?;
    if let Some(p) = part {
        super::picture::drop_if_unused(tx, &p)?;
    }
    Ok(())
}

/// Remove track `id` under `el`; its relationship id goes to `rid`.
fn remove_track(el: &mut XmlElement, id: &str, rid: &mut Option<String>) {
    el.children.retain(|c| match c {
        XmlNode::Element(e) if e.local() == "track" && e.attr("id") == Some(id) => {
            *rid = e
                .attrs
                .iter()
                .find(|(k, _)| k.ends_with(":embed") || k.ends_with(":link"))
                .map(|(_, v)| v.clone());
            false
        }
        _ => true,
    });
    for c in &mut el.children {
        if let XmlNode::Element(e) = c {
            remove_track(e, id, rid);
        }
    }
}

fn cmd_bookmark_trigger(
    tx: &mut Tx<'_>,
    sel: &SlideSel,
    index: usize,
    media: u32,
    bookmark: Option<String>,
) -> R<()> {
    let (_, pic, media) = clip_at(tx.pres, sel, Some(media))?;
    if let Some(b) = &bookmark {
        let known = playback_of(tx.pres, sel.slide, &pic, media)
            .bookmarks
            .iter()
            .any(|x| &x.name == b);
        if !known {
            return Err(EditError::Refused("noBookmark"));
        }
    }
    edit_timing(tx, sel.slide, |t| {
        let a = t
            .animations
            .get_mut(index)
            .ok_or(EditError::Refused("noAnimation"))?;
        a.trigger_shape = Some(media);
        a.trigger_bookmark = bookmark;
        // The first effect of a trigger sequence starts with its trigger.
        if a.trigger == AnimTrigger::AfterPrevious {
            a.trigger = AnimTrigger::OnClick;
        }
        Ok(())
    })
}

impl crate::Engine {
    /// Run a media playback command.
    pub fn slide_media_command(&mut self, cmd: MediaCommand) -> Result<SlideEditResult> {
        let mut out = Outcome::default();
        {
            let pres = self.pres.as_mut().ok_or(EngineError::NoDocument)?;
            let ed = &mut self.slides;
            ed.clamp(pres);
            ed.text = None;
            ed.table = None;
            ed.mutate(pres, &mut out, |tx, sel| match cmd {
                MediaCommand::MediaPlayback {
                    id,
                    trim_start_ms,
                    trim_end_ms,
                    fade_in_ms,
                    fade_out_ms,
                    start,
                    volume,
                    mute,
                    slides,
                    looped,
                    rewind,
                    hide_when_stopped,
                    full_screen,
                } => cmd_playback(
                    tx,
                    sel,
                    id,
                    [trim_start_ms, trim_end_ms],
                    [fade_in_ms, fade_out_ms],
                    start.as_deref(),
                    &NodeSet {
                        volume,
                        mute,
                        slides,
                        looped,
                        rewind,
                        hide_when_stopped,
                        full_screen,
                    },
                ),
                MediaCommand::MediaBookmarkAdd { id, name, time_ms } => {
                    cmd_bookmark_add(tx, sel, id, name.as_deref(), time_ms)
                }
                MediaCommand::MediaBookmarkRemove { id, name } => {
                    cmd_bookmark_remove(tx, sel, id, &name)
                }
                MediaCommand::MediaCaptionAdd {
                    id,
                    text,
                    label,
                    lang,
                } => cmd_caption_add(tx, sel, id, &text, label.as_deref(), lang.as_deref()),
                MediaCommand::MediaCaptionRemove { id, track } => {
                    cmd_caption_remove(tx, sel, id, &track)
                }
                MediaCommand::MediaBookmarkTrigger {
                    index,
                    media,
                    bookmark,
                } => cmd_bookmark_trigger(tx, sel, index, media, bookmark),
            });
        }
        let mut r = self.finish_slide_command(out)?;
        r.collab = self.slide_collab_after_edit();
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_offsets() {
        // [MS-PPTX] §3 example: 18374.0515 ms.
        assert_eq!(parse_time_offset("18374.0515"), Some(18_374.051_5));
        assert_eq!(parse_time_offset("2s"), Some(2000.0));
        assert_eq!(parse_time_offset("1.5min"), Some(90_000.0));
        assert_eq!(parse_time_offset("1h"), Some(3_600_000.0));
        assert_eq!(parse_time_offset("500µs"), Some(0.5));
        assert_eq!(parse_time_offset("12ms"), Some(12.0));
        assert_eq!(parse_time_offset("x"), None);
        assert_eq!(parse_time_offset("3 days"), None);
        assert_eq!(format_time_offset(1500.0), "1500");
        assert_eq!(format_time_offset(18_374.051_5), "18374.0515");
        assert_eq!(format_time_offset(0.0), "0");
        assert_eq!(format_time_offset(-3.0), "0");
    }

    #[test]
    fn webvtt_signature() {
        assert!(is_webvtt("WEBVTT\n\n00:01.000 --> 00:02.000\nHi"));
        assert!(is_webvtt("\u{feff}WEBVTT - title\n"));
        assert!(is_webvtt("WEBVTT"));
        assert!(!is_webvtt("WEBVTTX\n"));
        assert!(!is_webvtt("1\n00:00:01,000 --> 00:00:02,000\nSRT"));
    }
}
