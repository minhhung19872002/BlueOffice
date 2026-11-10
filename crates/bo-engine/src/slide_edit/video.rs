//! Export of the slide show as a video or an animated GIF (BlueSlide "Export video").
//!
//! A deterministic frame generator: the show is laid out on a timeline (per slide: its
//! transition, then its click steps played one after the other, held for the slide's
//! "advance after" time (`p:transition@advTm`, ECMA-376 Part 1, §19.3.1.50) or a default
//! time per slide), and frame `n` is the composited show at `n / fps` seconds. Slides are
//! drawn from the slide show layers (backdrop + one layer per animated shape, §19.5), so
//! entrance / exit / emphasis effects and transitions are sampled by moving, fading and
//! clipping rendered layers. Every host (web through WASM, desktop, CLI) shares it.
//!
//! The effects are BlueSlide's own simplified renditions: fade, push, cover, pull, wipe,
//! split, blinds, circle, dissolve and cut are drawn; other transitions (3-D, morph, …)
//! play as a cross-fade, effects other than fade / fly / wipe / appear as a fade.

use super::show::{AnimOut, TransitionOut};
use crate::{Engine, EngineError, Result};
use bo_raster::compose::{Canvas, Clip, Place};
use bo_raster::gif::{GifEncoder, GifOptions};
use bo_raster::RasterPixmap as Pixmap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Largest output width accepted (pixels).
pub const MAX_WIDTH: u32 = 3840;
/// Largest frame rate accepted.
pub const MAX_FPS: u32 = 60;
/// Largest frame rate of a GIF (100 / fps must stay a useful delay).
pub const MAX_GIF_FPS: u32 = 15;

/// Options of a video export.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct VideoOptions {
    /// Output width in pixels (height from the slide aspect ratio; both even).
    pub width: u32,
    /// Frames per second.
    pub fps: u32,
    /// Time on each slide without its own timing, in seconds.
    pub seconds_per_slide: f64,
    /// Use the slides' "advance after" times when they have one.
    pub use_timings: bool,
    /// GIF: loop forever (otherwise play once).
    #[serde(rename = "loop")]
    pub looped: bool,
    /// GIF: Floyd–Steinberg dithering.
    pub dither: bool,
}

impl Default for VideoOptions {
    fn default() -> Self {
        Self {
            width: 1280,
            fps: 30,
            seconds_per_slide: 5.0,
            use_timings: true,
            looped: true,
            dither: true,
        }
    }
}

/// Where one slide sits on the video timeline (ms).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideSpan {
    /// 0-based slide shown (hidden slides are left out of the video).
    pub slide: usize,
    /// Start of the slide's transition.
    pub start_ms: u64,
    /// Duration of the transition into the slide.
    pub transition_ms: u64,
    /// Whole time of the slide, transition included.
    pub length_ms: u64,
}

/// Size and length of the video.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Frames per second.
    pub fps: u32,
    /// Frames of the video (`ceil(duration × fps)`).
    pub frame_count: u32,
    /// Length of the show in ms.
    pub duration_ms: u64,
    /// Timeline of each slide.
    pub slides: Vec<SlideSpan>,
}

/// What the timeline needs of a slide.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SlideTiming {
    /// Transition into the slide (kind, direction, ms).
    pub transition: Option<(String, Option<String>, u32)>,
    /// "Advance after" time (ms).
    pub advance_after_ms: Option<u32>,
    /// Duration of each click step (0 = with the slide).
    pub steps: Vec<u32>,
}

/// Duration of a transition as the video plays it (a cut is instant unless through black).
fn transition_ms(t: Option<&(String, Option<String>, u32)>) -> u64 {
    match t {
        None => 0,
        Some((kind, dir, _)) if kind == "cut" && dir.as_deref() != Some("black") => 0,
        Some((_, _, ms)) => u64::from((*ms).max(1)),
    }
}

/// Lay the slides out on the timeline.
pub fn timeline(slides: &[SlideTiming], opts: &VideoOptions) -> (Vec<SlideSpan>, u64) {
    let default_ms = (opts.seconds_per_slide.max(0.0) * 1000.0).round() as u64;
    let mut at = 0u64;
    let spans = slides
        .iter()
        .map(|s| {
            let tr = transition_ms(s.transition.as_ref());
            let dwell = match (opts.use_timings, s.advance_after_ms) {
                (true, Some(ms)) => u64::from(ms),
                _ => default_ms,
            };
            let anims: u64 = s.steps.iter().map(|&d| u64::from(d)).sum();
            let len = tr + dwell.max(anims).max(1);
            let span = SlideSpan {
                slide: 0,
                start_ms: at,
                transition_ms: tr,
                length_ms: len,
            };
            at += len;
            span
        })
        .collect();
    (spans, at)
}

/// Frames of a show of `duration_ms` at `fps`.
pub fn frame_count(duration_ms: u64, fps: u32) -> u32 {
    (duration_ms * u64::from(fps)).div_ceil(1000) as u32
}

/// Even frame size for an output `width` and a slide of `w × h` points.
fn frame_size(width: u32, w: f64, h: f64) -> (u32, u32) {
    let fw = (width.clamp(16, MAX_WIDTH) / 2) * 2;
    let fh = ((f64::from(fw) * h / w.max(1.0) / 2.0).round() as u32 * 2).max(2);
    (fw, fh)
}

/// One slide prepared for the export.
struct SlideData {
    layers: Vec<bo_render::PageList>,
    anims: Vec<AnimOut>,
    transition: Option<TransitionOut>,
    timing: SlideTiming,
}

/// An export in progress (kept by the engine between frame requests).
pub struct VideoSession {
    opts: VideoOptions,
    info: VideoInfo,
    scale: f32,
    /// Slide size in points.
    size: (f64, f64),
    slides: Vec<SlideData>,
    /// Rendered layers of the last slides drawn.
    cache: HashMap<usize, Vec<Pixmap>>,
    gif: Option<GifEncoder>,
    /// GIF frame waiting for its delay (identical frames are merged).
    pending: Option<(Vec<u8>, u32)>,
    /// GIF time written so far (cs), so delays add up to the timeline exactly.
    gif_cs: u64,
}

/// Steps of a slide: duration of each click step (`0..=clicks`).
fn steps(anims: &[AnimOut], clicks: usize) -> Vec<u32> {
    (0..=clicks)
        .map(|k| {
            anims
                .iter()
                .filter(|a| a.click == k && a.trigger_shape.is_none())
                .map(|a| a.end_ms)
                .max()
                .unwrap_or(0)
        })
        .collect()
}

impl Engine {
    /// Start a video / GIF export of the open presentation.
    pub fn video_begin(&mut self, opts: VideoOptions) -> Result<VideoInfo> {
        let count = self.slide_count();
        let pres = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
        if count == 0 {
            return Err(EngineError::BadCommand("no slides".into()));
        }
        let (w, h) = super::slide_size(pres);
        let mut opts = opts;
        opts.fps = opts.fps.clamp(1, MAX_FPS);
        let (fw, fh) = frame_size(opts.width, w, h);
        opts.width = fw;
        let mut slides = Vec::with_capacity(count);
        let mut shown = Vec::with_capacity(count);
        for i in 0..count {
            let hidden = self
                .pres
                .as_ref()
                .and_then(|p| p.slides.get(i))
                .is_some_and(|s| s.show == Some(false));
            if hidden {
                continue;
            }
            let show = self
                .slide_show(i)
                .ok_or_else(|| EngineError::BadCommand(format!("slide {i}")))?;
            let timing = SlideTiming {
                transition: show
                    .transition
                    .as_ref()
                    .map(|t| (t.kind.clone(), t.dir.clone(), t.dur_ms)),
                advance_after_ms: show.transition.as_ref().and_then(|t| t.advance_after_ms),
                steps: steps(&show.animations, show.clicks),
            };
            shown.push(i);
            // A recorded narration's icon is hidden in the show (showWhenStopped="0").
            let narration: Vec<u32> = show
                .media
                .iter()
                .filter(|m| m.recorded == Some(super::record::RecKind::Narration))
                .map(|m| m.id)
                .collect();
            slides.push(SlideData {
                layers: show
                    .layers
                    .into_iter()
                    .map(|l| {
                        let hidden = !l.backdrop
                            && !l.ids.is_empty()
                            && l.ids.iter().all(|id| narration.contains(id));
                        if hidden {
                            bo_render::PageList {
                                commands: Vec::new(),
                                ..l.page
                            }
                        } else {
                            l.page
                        }
                    })
                    .collect(),
                anims: show.animations,
                transition: show.transition,
                timing,
            });
        }
        if slides.is_empty() {
            return Err(EngineError::BadCommand("no slides".into()));
        }
        let timings: Vec<SlideTiming> = slides.iter().map(|s| s.timing.clone()).collect();
        let (mut spans, duration_ms) = timeline(&timings, &opts);
        for (s, i) in spans.iter_mut().zip(&shown) {
            s.slide = *i;
        }
        let info = VideoInfo {
            width: fw,
            height: fh,
            fps: opts.fps,
            frame_count: frame_count(duration_ms, opts.fps),
            duration_ms,
            slides: spans,
        };
        self.video = Some(Box::new(VideoSession {
            scale: fw as f32 / w.max(1.0) as f32,
            size: (w, h),
            opts,
            info: info.clone(),
            slides,
            cache: HashMap::new(),
            gif: None,
            pending: None,
            gif_cs: 0,
        }));
        Ok(info)
    }

    /// JSON variant of [`Engine::video_begin`].
    pub fn video_begin_json(&mut self, opts: &str) -> Result<String> {
        let o: VideoOptions = if opts.trim().is_empty() {
            VideoOptions::default()
        } else {
            serde_json::from_str(opts).map_err(|e| EngineError::BadCommand(e.to_string()))?
        };
        let info = self.video_begin(o)?;
        serde_json::to_string(&info).map_err(|e| EngineError::BadCommand(e.to_string()))
    }

    /// Straight RGBA pixels of frame `index` of the export begun with [`Engine::video_begin`].
    pub fn video_frame(&mut self, index: u32) -> Result<Vec<u8>> {
        Ok(bo_raster::compose::to_rgba(
            &self.video_frame_pixmap(index)?,
        ))
    }

    /// Frame `index` as PNG.
    pub fn video_frame_png(&mut self, index: u32) -> Result<Vec<u8>> {
        self.video_frame_pixmap(index)?
            .encode_png()
            .map_err(|e| EngineError::Raster(bo_raster::RasterError::Encode(e.to_string())))
    }

    /// Render frame `index` and append it to the GIF of the export (identical
    /// consecutive frames become one longer frame).
    pub fn video_gif_add(&mut self, index: u32) -> Result<()> {
        let rgba = self.video_frame(index)?;
        let v = self.video.as_mut().ok_or(EngineError::NoDocument)?;
        if v.gif.is_none() {
            let opts = GifOptions {
                repeat: v.opts.looped.then_some(0),
                dither: v.opts.dither,
            };
            v.gif = Some(
                GifEncoder::new(v.info.width, v.info.height, opts)
                    .map_err(|e| EngineError::BadCommand(e.to_string()))?,
            );
        }
        // The frame lasts until the next frame time (in 1/100 s, rounded on the timeline).
        let fps = u64::from(v.info.fps);
        let end_cs = (u64::from(index) + 1) * 100 / fps;
        let start_cs = u64::from(index) * 100 / fps;
        let cs = (end_cs - start_cs) as u32;
        match &mut v.pending {
            Some((prev, d)) if *prev == rgba => *d += cs,
            _ => {
                flush_gif(v)?;
                v.pending = Some((rgba, cs));
            }
        }
        Ok(())
    }

    /// Finish the GIF of the export and return its bytes.
    pub fn video_gif_finish(&mut self) -> Result<Vec<u8>> {
        let v = self.video.as_mut().ok_or(EngineError::NoDocument)?;
        flush_gif(v)?;
        let gif = v
            .gif
            .take()
            .ok_or_else(|| EngineError::BadCommand("no GIF frames".into()))?;
        v.gif_cs = 0;
        Ok(gif.finish())
    }

    /// The whole show as an animated GIF (CLI, server).
    pub fn export_show_gif(&mut self, opts: VideoOptions) -> Result<Vec<u8>> {
        let mut opts = opts;
        opts.fps = opts.fps.clamp(1, MAX_GIF_FPS);
        let info = self.video_begin(opts)?;
        for i in 0..info.frame_count {
            self.video_gif_add(i)?;
        }
        let gif = self.video_gif_finish();
        self.video_end();
        gif
    }

    /// End the export and free its frames.
    pub fn video_end(&mut self) {
        self.video = None;
    }

    fn video_frame_pixmap(&mut self, index: u32) -> Result<Pixmap> {
        let fonts = &self.fonts;
        let images = &self.images;
        let v = self.video.as_mut().ok_or(EngineError::NoDocument)?;
        if index >= v.info.frame_count {
            return Err(EngineError::PageOutOfRange(index as usize));
        }
        let t = f64::from(index) * 1000.0 / f64::from(v.info.fps);
        let (slide, local) = locate(&v.info.slides, t);
        // Keep the rendered layers of this slide and the one before it only.
        v.cache.retain(|k, _| *k == slide || *k + 1 == slide);
        for s in [slide.checked_sub(1), Some(slide)].into_iter().flatten() {
            if !v.cache.contains_key(&s) {
                let mut layers = Vec::new();
                for page in &v.slides[s].layers {
                    layers.push(bo_raster::render_layer(page, fonts, v.scale, images)?);
                }
                v.cache.insert(s, layers);
            }
        }
        let (fw, fh) = (v.info.width, v.info.height);
        let span = &v.info.slides[slide];
        let tr = span.transition_ms as f64;
        let draw_slide = |s: usize, anim_ms: Option<f64>| -> Result<Pixmap> {
            let data = &v.slides[s];
            let mut c = canvas(fw, fh, [255, 255, 255])?;
            let places = layer_places(data, anim_ms, v.size);
            if let Some(layers) = v.cache.get(&s) {
                for (pm, p) in layers.iter().zip(&places) {
                    c.draw(pm, p);
                }
            }
            Ok(c.into_pixmap())
        };
        let mut frame = canvas(fw, fh, [0, 0, 0])?;
        if local < tr {
            let p = (local / tr) as f32;
            let incoming = draw_slide(slide, Some(0.0))?;
            let outgoing = match slide.checked_sub(1) {
                Some(prev) => Some(draw_slide(prev, None)?),
                None => None,
            };
            let (kind, dir) = v.slides[slide]
                .transition
                .as_ref()
                .map_or(("fade", ""), |t| {
                    (t.kind.as_str(), t.dir.as_deref().unwrap_or(""))
                });
            for (which, place) in transition_places(kind, dir, p, slide) {
                let src = match which {
                    Which::Out => outgoing.as_ref(),
                    Which::In => Some(&incoming),
                };
                if let Some(src) = src {
                    frame.draw(src, &place);
                }
            }
        } else {
            let pm = draw_slide(slide, Some(local - tr))?;
            frame.draw(&pm, &Place::default());
        }
        Ok(frame.into_pixmap())
    }
}

fn canvas(w: u32, h: u32, rgb: [u8; 3]) -> Result<Canvas> {
    Canvas::new(w, h, rgb).ok_or(EngineError::Raster(bo_raster::RasterError::InvalidSize(
        w, h,
    )))
}

fn flush_gif(v: &mut VideoSession) -> Result<()> {
    let Some((rgba, mut cs)) = v.pending.take() else {
        return Ok(());
    };
    let Some(gif) = v.gif.as_mut() else {
        return Ok(());
    };
    // A delay is 16 bits: very long frames are written several times.
    loop {
        let d = cs.min(u32::from(u16::MAX));
        gif.add_frame(&rgba, d as u16)
            .map_err(|e| EngineError::BadCommand(e.to_string()))?;
        v.gif_cs += u64::from(d);
        cs -= d;
        if cs == 0 {
            return Ok(());
        }
    }
}

/// Slide shown at `t` ms and the time since its start.
fn locate(spans: &[SlideSpan], t: f64) -> (usize, f64) {
    let mut i = 0;
    for (k, s) in spans.iter().enumerate() {
        if t >= s.start_ms as f64 {
            i = k;
        }
    }
    let start = spans.get(i).map_or(0.0, |s| s.start_ms as f64);
    (i, t - start)
}

// ------------------------------------------------------------------ effects

/// Placement of every layer of a slide `anim_ms` after its transition (`None` = once
/// all the effects are done).
fn layer_places(data: &SlideData, anim_ms: Option<f64>, size: (f64, f64)) -> Vec<Place> {
    let n = data.layers.len();
    let mut places = vec![Place::default(); n];
    // A layer whose first effect is an entrance starts hidden.
    let mut seen = vec![false; n];
    for a in &data.anims {
        if let Some(l) = a.layer.filter(|&l| l < n) {
            if !seen[l] {
                seen[l] = true;
                if a.class == "entr" {
                    places[l].opacity = 0.0;
                }
            }
        }
    }
    // Click step starts.
    let mut starts = Vec::with_capacity(data.timing.steps.len());
    let mut at = 0.0;
    for &d in &data.timing.steps {
        starts.push(at);
        at += f64::from(d);
    }
    let now = anim_ms.unwrap_or(f64::INFINITY);
    for a in &data.anims {
        let Some(l) = a.layer.filter(|&l| l < n) else {
            continue;
        };
        if a.trigger_shape.is_some() {
            continue;
        }
        let Some(&s) = starts.get(a.click) else {
            continue;
        };
        let (start, end) = (s + f64::from(a.start_ms), s + f64::from(a.end_ms));
        if now >= end {
            match a.class {
                "entr" => places[l] = Place::default(),
                "exit" => places[l].opacity = 0.0,
                _ => {}
            }
        } else if now >= start {
            let p = ((now - start) / (end - start).max(1.0)) as f32;
            places[l] = effect_place(a, p, size);
        }
    }
    places
}

/// A layer during effect `a` at progress `p` (0..1).
fn effect_place(a: &AnimOut, p: f32, size: (f64, f64)) -> Place {
    let entr = a.class == "entr";
    if !entr && a.class != "exit" {
        // Emphasis, motion path: a short pulse of the layer.
        return Place {
            opacity: 1.0 - 0.65 * (1.0 - (2.0 * p - 1.0).abs()),
            ..Place::default()
        };
    }
    // Exit effects play the entrance backwards.
    let q = if entr { p } else { 1.0 - p };
    // The layer is the whole slide; the shape rectangle (points) as fractions of it.
    let rect = a
        .rect
        .filter(|r| r[2] > 0.0 && r[3] > 0.0)
        .map(|[x, y, w, h]| {
            let (sw, sh) = (size.0.max(1.0), size.1.max(1.0));
            [
                (x / sw) as f32,
                (y / sh) as f32,
                (w / sw) as f32,
                (h / sh) as f32,
            ]
        });
    match a.effect.as_str() {
        "appear" => Place {
            opacity: if entr { 0.0 } else { 1.0 },
            ..Place::default()
        },
        "flyIn" if rect.is_some() => {
            let [x, y, w, h] = rect.unwrap_or_default();
            let k = 1.0 - q;
            let (dx, dy) = match a.direction.as_deref() {
                Some("l") => (-(x + w) * k, 0.0),
                Some("r") => ((1.0 - x) * k, 0.0),
                Some("t") => (0.0, -(y + h) * k),
                _ => (0.0, (1.0 - y) * k),
            };
            Place {
                dx,
                dy,
                ..Place::default()
            }
        }
        "wipe" if rect.is_some() => {
            let [x, y, w, h] = rect.unwrap_or_default();
            let r = match a.direction.as_deref() {
                Some("t") => [x, y, x + w, y + h * q],
                Some("l") => [x, y, x + w * q, y + h],
                Some("r") => [x + w * (1.0 - q), y, x + w, y + h],
                _ => [x, y + h * (1.0 - q), x + w, y + h],
            };
            Place {
                clip: Some(Clip::Rects(vec![r])),
                ..Place::default()
            }
        }
        _ => Place {
            opacity: q,
            ..Place::default()
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    Out,
    In,
}

/// Motion of a transition towards side `d` (`l`, `r`, `u`, `d`, diagonals `lu`, `rd`, …).
fn motion(d: &str) -> (f32, f32) {
    let d = if d.is_empty() { "l" } else { d };
    let dx = if d.contains('l') {
        -1.0
    } else if d.contains('r') {
        1.0
    } else {
        0.0
    };
    let dy = if d.contains('u') {
        -1.0
    } else if d.contains('d') {
        1.0
    } else {
        0.0
    };
    (dx, dy)
}

/// Transitions `random` picks from (by the slide index, as the slide show does).
const RANDOM_POOL: [&str; 8] = [
    "fade", "push", "wipe", "split", "blinds", "circle", "dissolve", "cover",
];

/// Layers of a transition frame at progress `p`, bottom first, over black.
fn transition_places(kind: &str, dir: &str, p: f32, seed: usize) -> Vec<(Which, Place)> {
    let kind = if kind == "random" {
        RANDOM_POOL[seed % RANDOM_POOL.len()]
    } else {
        kind
    };
    let full = Place::default();
    let clip = |rects: Vec<[f32; 4]>| Place {
        clip: Some(Clip::Rects(rects)),
        ..Place::default()
    };
    let fade_in = vec![
        (Which::Out, full.clone()),
        (
            Which::In,
            Place {
                opacity: p,
                ..Place::default()
            },
        ),
    ];
    match kind {
        "cut" => {
            if p < 0.5 {
                vec![]
            } else {
                vec![(Which::In, full)]
            }
        }
        "fade" if dir == "black" => {
            if p < 0.5 {
                vec![(
                    Which::Out,
                    Place {
                        opacity: 1.0 - 2.0 * p,
                        ..Place::default()
                    },
                )]
            } else {
                vec![(
                    Which::In,
                    Place {
                        opacity: 2.0 * p - 1.0,
                        ..Place::default()
                    },
                )]
            }
        }
        "push" => {
            let (vx, vy) = motion(dir);
            vec![
                (
                    Which::Out,
                    Place {
                        dx: vx * p,
                        dy: vy * p,
                        ..Place::default()
                    },
                ),
                (
                    Which::In,
                    Place {
                        dx: -vx * (1.0 - p),
                        dy: -vy * (1.0 - p),
                        ..Place::default()
                    },
                ),
            ]
        }
        "cover" => {
            let (vx, vy) = motion(dir);
            vec![
                (Which::Out, full),
                (
                    Which::In,
                    Place {
                        dx: -vx * (1.0 - p),
                        dy: -vy * (1.0 - p),
                        ..Place::default()
                    },
                ),
            ]
        }
        "pull" => {
            let (vx, vy) = motion(dir);
            vec![
                (Which::In, full),
                (
                    Which::Out,
                    Place {
                        dx: vx * p,
                        dy: vy * p,
                        ..Place::default()
                    },
                ),
            ]
        }
        "wipe" => {
            let r = match dir {
                "r" => [0.0, 0.0, p, 1.0],
                "u" => [0.0, 1.0 - p, 1.0, 1.0],
                "d" => [0.0, 0.0, 1.0, p],
                _ => [1.0 - p, 0.0, 1.0, 1.0],
            };
            vec![(Which::Out, full), (Which::In, clip(vec![r]))]
        }
        "split" => {
            let vert = dir.starts_with("vert");
            let rects = if dir.ends_with("In") {
                let e = p / 2.0;
                if vert {
                    vec![[0.0, 0.0, e, 1.0], [1.0 - e, 0.0, 1.0, 1.0]]
                } else {
                    vec![[0.0, 0.0, 1.0, e], [0.0, 1.0 - e, 1.0, 1.0]]
                }
            } else if vert {
                vec![[0.5 - p / 2.0, 0.0, 0.5 + p / 2.0, 1.0]]
            } else {
                vec![[0.0, 0.5 - p / 2.0, 1.0, 0.5 + p / 2.0]]
            };
            vec![(Which::Out, full), (Which::In, clip(rects))]
        }
        "blinds" => {
            const N: usize = 6;
            let rects = (0..N)
                .map(|k| {
                    let a = k as f32 / N as f32;
                    let b = a + p / N as f32;
                    if dir == "vert" {
                        [a, 0.0, b, 1.0]
                    } else {
                        [0.0, a, 1.0, b]
                    }
                })
                .collect();
            vec![(Which::Out, full), (Which::In, clip(rects))]
        }
        "circle" => vec![
            (Which::Out, full),
            (
                Which::In,
                Place {
                    clip: Some(Clip::Circle {
                        cx: 0.5,
                        cy: 0.5,
                        r: p * 0.5,
                    }),
                    ..Place::default()
                },
            ),
        ],
        "dissolve" => {
            const COLS: usize = 16;
            const ROWS: usize = 9;
            let rank = ranks(COLS * ROWS, seed as u32);
            let shown = (p * (COLS * ROWS) as f32).round() as usize;
            let rects = (0..COLS * ROWS)
                .filter(|&i| rank[i] < shown)
                .map(|i| {
                    let (c, r) = ((i % COLS) as f32, (i / COLS) as f32);
                    let (w, h) = (1.0 / COLS as f32, 1.0 / ROWS as f32);
                    [c * w, r * h, (c + 1.0) * w, (r + 1.0) * h]
                })
                .collect();
            vec![(Which::Out, full), (Which::In, clip(rects))]
        }
        // Fade and every effect drawn as a cross-fade.
        _ => fade_in,
    }
}

/// A deterministic permutation (rank of each of `n` items) from `seed`.
fn ranks(n: usize, seed: u32) -> Vec<usize> {
    let mut order: Vec<usize> = (0..n).collect();
    let mut s = seed.wrapping_mul(2_654_435_761).wrapping_add(12_345);
    for i in (1..n).rev() {
        s = s.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let j = s as usize % (i + 1);
        order.swap(i, j);
    }
    let mut rank = vec![0; n];
    for (r, &item) in order.iter().enumerate() {
        rank[item] = r;
    }
    rank
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(secs: f64, fps: u32) -> VideoOptions {
        VideoOptions {
            seconds_per_slide: secs,
            fps,
            ..VideoOptions::default()
        }
    }

    #[test]
    fn timeline_uses_timings_transitions_and_steps() {
        let slides = vec![
            SlideTiming::default(),
            SlideTiming {
                transition: Some(("fade".into(), None, 500)),
                advance_after_ms: Some(1500),
                steps: vec![0, 400],
            },
            SlideTiming {
                transition: Some(("cut".into(), None, 300)),
                advance_after_ms: None,
                steps: vec![3000, 2500],
            },
        ];
        let (spans, total) = timeline(&slides, &opts(2.0, 10));
        assert_eq!(
            spans
                .iter()
                .map(|s| (s.start_ms, s.transition_ms, s.length_ms))
                .collect::<Vec<_>>(),
            vec![(0, 0, 2000), (2000, 500, 2000), (4000, 0, 5500)]
        );
        assert_eq!(total, 9500);
        assert_eq!(frame_count(total, 10), 95);
        assert_eq!(frame_count(1001, 30), 31);
        // Without timings every slide holds the default time (or its effects).
        let (spans, _) = timeline(
            &slides,
            &VideoOptions {
                use_timings: false,
                ..opts(1.0, 10)
            },
        );
        assert_eq!(spans[1].length_ms, 1500);
    }

    #[test]
    fn frame_sizes_are_even_and_keep_the_ratio() {
        assert_eq!(frame_size(1280, 960.0, 540.0), (1280, 720));
        assert_eq!(frame_size(481, 720.0, 540.0), (480, 360));
        assert_eq!(frame_size(1920, 960.0, 540.0), (1920, 1080));
    }

    #[test]
    fn transitions_move_from_the_outgoing_to_the_incoming_slide() {
        let at = |k: &str, d: &str, p: f32| transition_places(k, d, p, 0);
        // Push left: the incoming slide starts one width to the right.
        let s = at("push", "l", 0.0);
        assert_eq!(s[1].1.dx, 1.0);
        assert_eq!(at("push", "l", 1.0)[1].1.dx, 0.0);
        assert_eq!(at("fade", "", 0.25)[1].1.opacity, 0.25);
        assert_eq!(at("cut", "black", 0.2).len(), 0);
        assert_eq!(at("fade", "black", 0.25)[0].0, Which::Out);
        assert_eq!(at("fade", "black", 0.75)[0].0, Which::In);
        // Unknown effects cross-fade; random is stable per slide.
        assert_eq!(at("vortex", "", 0.5), at("fade", "", 0.5));
        assert_eq!(
            transition_places("random", "", 0.5, 3),
            transition_places("split", "", 0.5, 3)
        );
        let r = ranks(144, 7);
        let mut sorted = r.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..144).collect::<Vec<_>>());
        assert_eq!(r, ranks(144, 7));
    }
}
