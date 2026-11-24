//! BlueDoc parity round 22 (docs/ui/REPORT.md "Vòng 112"): the reference's thumbnail
//! galleries (WordArt Quick Styles, Text Effects › Transform, Picture Styles, the Picture /
//! Shape Effects categories — [`crate::doc_thumbs22`]), Shape Format › Text Direction ▾,
//! Text Effects › Transform (§20.1.9.19 prstTxWarp) and the Format Shape text box options
//! (`bo_engine::doc_shapestyle` `shapeTextLayout` / `shapeTextWarp` /
//! `shapeRotateWithShape`). A child module of the shell.
use super::DocWindow;
use crate::doc_thumbs22::{Cell, Section, Thumb, ThumbGallery};
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::RibbonHit;
use crate::Input;
use bo_engine::doc_picstyle as ps;
use bo_engine::doc_shapestyle as ss;
use bo_render::{PathSeg, Rgba};
use serde_json::json;

#[cfg(test)]
#[path = "doc_r22_tests.rs"]
mod tests;

/// Round-22 state of the window.
#[derive(Default)]
pub(super) struct R22 {
    /// The open thumbnail gallery.
    pub(super) gallery: Option<ThumbGallery>,
}

/// Commands of round 22 on the ribbon.
pub(crate) const IDS: [&str; 1] = ["shape.textDirection"];

/// Text Effects › Transform: Follow Path presets, then the Warp presets (§20.1.10.76).
pub(crate) const FOLLOW_PATH: [&str; 4] =
    ["textArchUp", "textArchDown", "textCircle", "textButton"];

/// Words of a preset name without its `text` prefix ("textArchUp" → "Arch Up").
fn words(name: &str) -> String {
    let name = name.strip_prefix("text").unwrap_or(name);
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if i == 0 {
            out.extend(c.to_uppercase());
            continue;
        }
        if c.is_ascii_uppercase()
            || (c.is_ascii_digit() && !out.ends_with(|d: char| d.is_ascii_digit()))
        {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

fn rgb(v: u32) -> Rgba {
    Rgba((v >> 16) as u8, (v >> 8) as u8, v as u8, 255)
}

fn hex(h: &str) -> Rgba {
    rgb(u32::from_str_radix(h, 16).unwrap_or(0))
}

fn alpha(c: Rgba, a: u8) -> Rgba {
    Rgba(c.0, c.1, c.2, a)
}

type Paths = Vec<crate::doc_thumbs22::ThumbPath>;

/// The corners (top-left, top-right, bottom-right, bottom-left) of a sample turned by a
/// 3-D Rotation preset (BlueOffice's own reading of the camera names).
fn corners(rotation: Option<&str>, b: [f32; 4]) -> [(f32, f32); 4] {
    let [x, y, w, h] = b;
    let mut c = [(x, y), (x + w, y), (x + w, y + h), (x, y + h)];
    let Some(r) = rotation else { return c };
    let k = 0.18 * w;
    if r.starts_with("isometric") {
        let up = r.contains("Right") || r.contains("Up");
        let dy = if up { -k } else { k };
        c[1].1 += dy;
        c[2].1 += dy;
        c[0].0 += k * 0.3;
        c[3].0 += k * 0.3;
    } else if r.starts_with("perspective") {
        if r.contains("Left") {
            c[0].1 += k * 0.6;
            c[3].1 -= k * 0.6;
        } else if r.contains("Right") {
            c[1].1 += k * 0.6;
            c[2].1 -= k * 0.6;
        } else if r.contains("Below") {
            c[2].0 -= k * 0.6;
            c[3].0 += k * 0.6;
        } else {
            c[0].0 += k * 0.6;
            c[1].0 -= k * 0.6;
        }
    } else if r.starts_with("oblique") {
        let dx = if r.contains("Left") { -k } else { k };
        let dy = if r.contains("Top") { -k * 0.6 } else { k * 0.6 };
        c[1].0 += dx * 0.5;
        c[1].1 += dy;
        c[2].0 += dx * 0.5;
    }
    c
}

/// Bilinear point (`u`, `v`) of a quad.
fn at(q: &[(f32, f32); 4], u: f32, v: f32) -> (f32, f32) {
    let top = (
        q[0].0 + (q[1].0 - q[0].0) * u,
        q[0].1 + (q[1].1 - q[0].1) * u,
    );
    let bot = (
        q[3].0 + (q[2].0 - q[3].0) * u,
        q[3].1 + (q[2].1 - q[3].1) * u,
    );
    (top.0 + (bot.0 - top.0) * v, top.1 + (bot.1 - top.1) * v)
}

fn quad_segs(q: &[(f32, f32); 4]) -> Vec<PathSeg> {
    vec![
        PathSeg::M(q[0].0, q[0].1),
        PathSeg::L(q[1].0, q[1].1),
        PathSeg::L(q[2].0, q[2].1),
        PathSeg::L(q[3].0, q[3].1),
        PathSeg::Z,
    ]
}

/// An ellipse (or rounded rectangle) inside a quad's (u, v) space.
fn round_segs(q: &[(f32, f32); 4], round: f32) -> Vec<PathSeg> {
    let n = 24;
    let mut out = Vec::new();
    for i in 0..=n {
        let a = std::f32::consts::TAU * i as f32 / n as f32;
        let (s, c) = a.sin_cos();
        // A superellipse: round = 1 → ellipse, smaller → squarer.
        let e = 2.0 / (1.0 + 3.0 * (1.0 - round));
        let u = 0.5 + 0.5 * c.signum() * c.abs().powf(e);
        let v = 0.5 + 0.5 * s.signum() * s.abs().powf(e);
        let (x, y) = at(q, u, v);
        out.push(if i == 0 {
            PathSeg::M(x, y)
        } else {
            PathSeg::L(x, y)
        });
    }
    out.push(PathSeg::Z);
    out
}

/// What an effect thumbnail shows.
#[derive(Debug, Clone, Copy, Default)]
struct Fx<'a> {
    picture: bool,
    shadow: Option<&'a str>,
    reflection: Option<&'a str>,
    glow: Option<(f64, u32)>,
    soft: Option<f64>,
    bevel: Option<&'a str>,
    rotation: Option<&'a str>,
    border: Option<(u32, f64)>,
    shape: Option<&'a str>,
}

/// A sample (a little landscape picture, or an accent square for shapes) with effects.
fn effect_thumb(fx: Fx<'_>) -> Thumb {
    let mut out: Paths = Vec::new();
    let base = [0.18, 0.12, 0.64, 0.5];
    let q = corners(fx.rotation, base);
    let outline = |q: &[(f32, f32); 4]| match fx.shape {
        Some("ellipse") => round_segs(q, 1.0),
        Some("roundRect") => round_segs(q, 0.35),
        _ => quad_segs(q),
    };
    let grow = |q: &[(f32, f32); 4], d: f32| -> [(f32, f32); 4] {
        [
            (q[0].0 - d, q[0].1 - d),
            (q[1].0 + d, q[1].1 - d),
            (q[2].0 + d, q[2].1 + d),
            (q[3].0 - d, q[3].1 + d),
        ]
    };
    let shift = |q: &[(f32, f32); 4], dx: f32, dy: f32| q.map(|(x, y)| (x + dx, y + dy));
    // Glow and outer shadow behind the sample.
    if let Some((size, c)) = fx.glow {
        let d = (size as f32 * 0.006).clamp(0.02, 0.12);
        out.push((outline(&grow(&q, d)), Some(alpha(rgb(c), 150)), None));
    }
    if let Some(name) = fx.shadow.filter(|s| s.starts_with("outer")) {
        let (_, _, dir, dist, _) = ps::SHADOWS
            .iter()
            .find(|s| s.0 == name)
            .copied()
            .unwrap_or(("", false, 45.0, 3.0, 4.0));
        let (s, c) = (dir as f32).to_radians().sin_cos();
        let d = if dist > 0.0 { 0.06 } else { 0.0 };
        let sq = if dist > 0.0 {
            shift(&q, c * d, s * d)
        } else {
            grow(&q, 0.04)
        };
        out.push((outline(&sq), Some(Rgba(0, 0, 0, 90)), None));
    }
    // The sample.
    if fx.picture {
        out.push((outline(&q), Some(rgb(0x9D_C3E6)), None));
        // A hill and a sun (own drawing).
        let hill: Vec<PathSeg> = {
            let pts = [
                (0.0, 1.0),
                (0.0, 0.7),
                (0.35, 0.45),
                (0.6, 0.65),
                (1.0, 0.5),
                (1.0, 1.0),
            ];
            let mut v: Vec<PathSeg> = pts
                .iter()
                .enumerate()
                .map(|(i, (u, w))| {
                    let (x, y) = at(&q, *u, *w);
                    if i == 0 {
                        PathSeg::M(x, y)
                    } else {
                        PathSeg::L(x, y)
                    }
                })
                .collect();
            v.push(PathSeg::Z);
            v
        };
        out.push((hill, Some(rgb(0x70_AD47)), None));
        let sun = [
            at(&q, 0.7, 0.15),
            at(&q, 0.85, 0.15),
            at(&q, 0.85, 0.35),
            at(&q, 0.7, 0.35),
        ];
        out.push((round_segs(&sun, 1.0), Some(rgb(0xFF_C000)), None));
    } else {
        out.push((outline(&q), Some(rgb(0x44_72C4)), None));
    }
    // Inner shadow: dark bands on the sides away from the light.
    if let Some(name) = fx.shadow.filter(|s| s.starts_with("inner")) {
        let band = 0.12;
        let dark = Rgba(0, 0, 0, 80);
        let sides: &[(f32, f32, f32, f32)] = match name {
            "innerCenter" => &[
                (0.0, 0.0, 1.0, band),
                (0.0, 1.0 - band, 1.0, band),
                (0.0, 0.0, band, 1.0),
                (1.0 - band, 0.0, band, 1.0),
            ],
            n if n.contains("Top") && n.contains("Left") => {
                &[(0.0, 0.0, 1.0, band), (0.0, 0.0, band, 1.0)]
            }
            n if n.contains("Top") && n.contains("Right") => {
                &[(0.0, 0.0, 1.0, band), (1.0 - band, 0.0, band, 1.0)]
            }
            n if n.contains("Bottom") && n.contains("Left") => {
                &[(0.0, 1.0 - band, 1.0, band), (0.0, 0.0, band, 1.0)]
            }
            n if n.contains("Bottom") && n.contains("Right") => {
                &[(0.0, 1.0 - band, 1.0, band), (1.0 - band, 0.0, band, 1.0)]
            }
            n if n.contains("Top") => &[(0.0, 0.0, 1.0, band)],
            n if n.contains("Bottom") => &[(0.0, 1.0 - band, 1.0, band)],
            n if n.contains("Left") => &[(0.0, 0.0, band, 1.0)],
            _ => &[(1.0 - band, 0.0, band, 1.0)],
        };
        for (u, v, w, h) in sides {
            let r = [
                at(&q, *u, *v),
                at(&q, u + w, *v),
                at(&q, u + w, v + h),
                at(&q, *u, v + h),
            ];
            out.push((quad_segs(&r), Some(dark), None));
        }
    }
    // Soft edges: pale frames fading inwards.
    if let Some(r) = fx.soft {
        let steps = 3;
        let depth = ((r as f32) / 50.0 * 0.35).clamp(0.04, 0.35);
        for i in 0..steps {
            let t = depth * (steps - i) as f32 / steps as f32;
            let inner = [
                at(&q, t, t),
                at(&q, 1.0 - t, t),
                at(&q, 1.0 - t, 1.0 - t),
                at(&q, t, 1.0 - t),
            ];
            let mut segs = quad_segs(&grow(&q, 0.005));
            // The frame between the outline and `inner` (even-odd holes are not needed:
            // translucent white over the edge).
            segs.extend(quad_segs(&[inner[0], inner[3], inner[2], inner[1]]));
            out.push((segs, Some(Rgba(255, 255, 255, 70)), None));
        }
    }
    // Bevel: a light top-left facet and a dark bottom-right one.
    if fx.bevel.is_some() {
        let t = 0.12;
        let light = [q[0], q[1], at(&q, 1.0 - t, t), at(&q, t, t)];
        let light2 = [q[0], at(&q, t, t), at(&q, t, 1.0 - t), q[3]];
        let dark = [at(&q, t, 1.0 - t), at(&q, 1.0 - t, 1.0 - t), q[2], q[3]];
        let dark2 = [at(&q, 1.0 - t, t), q[1], q[2], at(&q, 1.0 - t, 1.0 - t)];
        out.push((quad_segs(&light), Some(Rgba(255, 255, 255, 110)), None));
        out.push((quad_segs(&light2), Some(Rgba(255, 255, 255, 80)), None));
        out.push((quad_segs(&dark), Some(Rgba(0, 0, 0, 70)), None));
        out.push((quad_segs(&dark2), Some(Rgba(0, 0, 0, 50)), None));
    }
    if let Some((c, w)) = fx.border.filter(|b| b.1 > 0.0) {
        out.push((
            outline(&q),
            None,
            Some((rgb(c), (w as f32 * 0.35).clamp(1.0, 3.0))),
        ));
    } else if fx.picture {
        out.push((outline(&q), None, Some((Rgba(0x80, 0x80, 0x80, 255), 0.5))));
    }
    // Reflection: a faded copy under the sample.
    if let Some(name) = fx.reflection {
        let (_, end, dist) = ps::REFLECTIONS
            .iter()
            .find(|r| r.0 == name)
            .copied()
            .unwrap_or(("", 55, 0.0));
        let gap = dist as f32 * 0.006;
        let len = 0.5 * end as f32 / 100.0;
        let top = q[3].1.max(q[2].1) + gap;
        let r = [
            (q[3].0, top),
            (q[2].0, top),
            (q[2].0, top + len.min(0.34)),
            (q[3].0, top + len.min(0.34)),
        ];
        let c = if fx.picture {
            rgb(0x70_AD47)
        } else {
            rgb(0x44_72C4)
        };
        out.push((quad_segs(&r), Some(alpha(c, 90)), None));
    }
    Thumb::Paths(out)
}

/// A WordArt quick style as a letter.
fn wordart_thumb(i: usize) -> Thumb {
    let Some(&(fill, outline, shadow, glow, _refl, grad)) = ss::WORDART.get(i) else {
        return Thumb::None;
    };
    let fill = fill
        .map(hex)
        .or_else(|| grad.map(|g| hex(g.0)))
        .unwrap_or(Rgba(255, 255, 255, 255));
    Thumb::Letter {
        fill,
        outline: outline
            .map(|o| hex(o.0))
            .or_else(|| (fill == Rgba(255, 255, 255, 255)).then_some(Rgba(0x40, 0x40, 0x40, 255))),
        halo: glow
            .map(|g| alpha(hex(g), 170))
            .or_else(|| shadow.then_some(Rgba(0, 0, 0, 60))),
    }
}

/// The warp previews of the engine as thumbnails (the guide shapes of each preset).
fn warp_thumbs() -> Vec<(String, Thumb)> {
    let previews = bo_engine::slide_edit::effects::warp_previews();
    let (w, h) = bo_engine::slide_edit::effects::PREVIEW_BOX;
    previews
        .into_iter()
        .map(|p| {
            let segs = crate::doc_thumbs22::parse_d(&p.d, 1.0 / w as f32, 1.0 / h as f32);
            (
                p.name,
                Thumb::Paths(vec![(segs, Some(rgb(0x44_72C4)), None)]),
            )
        })
        .collect()
}

impl DocWindow {
    fn r22_t(&self, key: &str, fallback: &str) -> String {
        let v = self.strings.get(key);
        if v == key {
            fallback.to_owned()
        } else {
            v.to_owned()
        }
    }

    fn r22_shape_style(&self) -> Option<bo_model::shape_style::ShapeStyle> {
        self.view
            .engine()
            .session()
            .and_then(bo_edit::Session::tools_shape)
            .and_then(|i| i.shape)
            .and_then(|s| s.style.map(|b| *b))
    }

    fn r22_run(&mut self, v: &serde_json::Value) -> bool {
        self.menu = None;
        self.r22.gallery = None;
        let ok = self.view.picture_command(v);
        self.sync_context();
        ok
    }

    /// Anchor of a ribbon command (the window centre when it is not shown).
    fn r22_anchor(&self, id: &str) -> [f32; 4] {
        crate::doc_tabs7::static_id(id)
            .or_else(|| IDS.iter().copied().find(|i| *i == id))
            .or_else(|| {
                [
                    "shape.wordArt",
                    "shape.textEffects",
                    "shape.effects",
                    "picture.quickStyles",
                    "picture.effects",
                ]
                .into_iter()
                .find(|i| *i == id)
            })
            .and_then(|sid| self.ribbon.rect_of(RibbonHit::Command(sid)))
            .unwrap_or([self.size.0 / 2.0, 120.0, 60.0, 30.0])
    }

    fn r22_open(
        &mut self,
        owner: &str,
        sections: Vec<Section>,
        footer: Vec<(String, String)>,
        grid: (usize, (f32, f32)),
    ) -> bool {
        self.menu = None;
        let anchor = self.r22_anchor(owner);
        self.r22.gallery = Some(ThumbGallery::open(
            sections, footer, grid, anchor, self.size, self.scale,
        ));
        true
    }

    /// A round-22 command; `None` when `id` is not one.
    pub(super) fn r22_action(&mut self, id: &str) -> Option<bool> {
        // A Quick Access Toolbar button changed with Modify… runs its command.
        if id.contains('|') {
            let (cmd, ..) = crate::doc_forms::options6::r21::split_quick(id);
            let cmd = cmd.to_owned();
            return Some(self.action(&cmd));
        }
        Some(match id {
            "shape.textDirection" => self.r22_text_direction(),
            _ if id.starts_with("r22.vert:") => self.r22_run(&json!({
                "cmd": "shapeTextLayout", "vert": &id["r22.vert:".len()..]
            })),
            _ if id.starts_with("r22.warp:") => self.r22_run(&json!({
                "cmd": "shapeTextWarp", "preset": &id["r22.warp:".len()..]
            })),
            "r22.transform" => self.r22_transform(),
            _ if id.starts_with("r22.fxOptions:") => {
                let section = match &id["r22.fxOptions:".len()..] {
                    "reflection" => "reflection",
                    "glow" => "glow",
                    "soft" => "soft",
                    "format3d" => "format3d",
                    "rotation3d" => "rotation3d",
                    _ => "shadow",
                };
                self.menu = None;
                self.r21_open_pane(crate::doc_fmtpane21::Page::Effects, Some(section), false)
            }
            "fp22.formatPicture" => {
                self.menu = None;
                self.r21_open_pane(crate::doc_fmtpane21::Page::Picture, Some("corr"), false)
            }
            "picture.layout" => self.r22_picture_layouts(),
            _ if id.starts_with("r22.picLayout:") => {
                let layout = id["r22.picLayout:".len()..].to_owned();
                self.r22_picture_layout(&layout)
            }
            // Insert › Object › Change Icon… over the Object dialog, and its answer.
            "r22.changeIcon" => {
                let Some((caption, style, file)) = self
                    .r3
                    .form
                    .as_ref()
                    .map(crate::doc_forms::DocForm::r22_icon_state)
                else {
                    return Some(false);
                };
                let mut c = self.form_context();
                c.extra["r22"] = json!({ "caption": caption, "style": style, "file": file });
                let f = crate::doc_forms::open(
                    crate::doc_forms::DocFormKind::ChangeIcon,
                    &self.strings,
                    &c,
                );
                self.r3.stash = self.r3.form.take();
                self.r3.form = Some(f);
                self.relayout_pending = true;
                true
            }
            _ if id.starts_with("r22.addins:") => {
                let which: u64 = id["r22.addins:".len()..].parse().unwrap_or(0);
                let mut c = self.form_context();
                c.extra["r22"] = json!({ "which": which });
                self.menu = None;
                self.r3.form = Some(crate::doc_forms::open(
                    crate::doc_forms::DocFormKind::AddinList,
                    &self.strings,
                    &c,
                ));
                self.relayout_pending = true;
                true
            }
            _ if id.starts_with("r22.iconSet:") => {
                let rest = &id["r22.iconSet:".len()..];
                let (style, rest) = rest.split_once(':').unwrap_or(("0", rest));
                let (caption, file) = rest.split_once('\u{1f}').unwrap_or((rest, ""));
                if let Some(f) = self.r3.form.as_mut() {
                    f.r22_set_icon(style.parse().unwrap_or(0), caption, file);
                }
                true
            }
            // Thumbnail galleries in place of the word lists of rounds 19–20.
            "shape.wordArt" => self.r22_wordart(),
            "picture.quickStyles" => self.r22_picture_styles(),
            _ if id.starts_with("r19.fxmenu:") => {
                let kind = id["r19.fxmenu:".len()..].to_owned();
                self.r22_effects("picture.effects", "r19.fx", &kind, true)
            }
            _ if id.starts_with("r20.menu:shape.effects/") => {
                let kind = id["r20.menu:shape.effects/".len()..].to_owned();
                self.r22_effects("shape.effects", "r20.fx", &kind, false)
            }
            _ => return None,
        })
    }

    /// Shape Format › Text Direction ▾: Horizontal, Rotate all text 90° / 270°.
    fn r22_text_direction(&mut self) -> bool {
        let vert = self.r22_shape_style().and_then(|s| s.vert);
        let items: Vec<MenuItem> = [
            ("horz", "doc22.horizontal", "Horizontal"),
            ("vert", "doc22.rotate90", "Rotate all text 90°"),
            ("vert270", "doc22.rotate270", "Rotate all text 270°"),
        ]
        .into_iter()
        .map(|(v, k, en)| MenuItem {
            label: self.r22_t(k, en),
            choice: Choice::Command(format!("r22.vert:{v}")),
            checked: vert.as_deref().unwrap_or("horz") == v,
            dim: false,
        })
        .collect();
        let anchor = self.r22_anchor("shape.textDirection");
        self.r22.gallery = None;
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
        true
    }

    /// Text Effects › Transform: No Transform, Follow Path and Warp galleries.
    fn r22_transform(&mut self) -> bool {
        let current = self.r22_shape_style().and_then(|s| s.warp);
        let cell = |name: String, thumb: Thumb| Cell {
            checked: current.as_deref() == Some(name.as_str()),
            command: format!("r22.warp:{name}"),
            tip: words(&name),
            thumb,
        };
        let (follow, warp): (Vec<_>, Vec<_>) = warp_thumbs()
            .into_iter()
            .partition(|(n, _)| FOLLOW_PATH.contains(&n.as_str()));
        let sections = vec![
            Section {
                title: self.r22_t("doc22.noTransform", "No Transform"),
                cells: vec![Cell {
                    thumb: Thumb::None,
                    command: "r22.warp:none".into(),
                    tip: self.r22_t("doc22.noTransform", "No Transform"),
                    checked: current.is_none(),
                }],
            },
            Section {
                title: self.r22_t("doc22.followPath", "Follow Path"),
                cells: follow.into_iter().map(|(n, t)| cell(n, t)).collect(),
            },
            Section {
                title: self.r22_t("doc22.warp", "Warp"),
                cells: warp.into_iter().map(|(n, t)| cell(n, t)).collect(),
            },
        ];
        self.r22_open("shape.textEffects", sections, Vec::new(), (4, (58.0, 50.0)))
    }

    /// Picture Format › Picture Layout ▾: the picture layouts of the SmartArt library.
    fn r22_picture_layouts(&mut self) -> bool {
        let sky = Some(rgb(0x9D_C3E6));
        let bar = Some(rgb(0x44_72C4));
        let r = |x: f32, y: f32, w: f32, h: f32| {
            vec![
                PathSeg::M(x, y),
                PathSeg::L(x + w, y),
                PathSeg::L(x + w, y + h),
                PathSeg::L(x, y + h),
                PathSeg::Z,
            ]
        };
        let cards = Thumb::Paths(vec![
            (r(0.05, 0.15, 0.42, 0.45), sky, None),
            (r(0.05, 0.6, 0.42, 0.2), bar, None),
            (r(0.53, 0.15, 0.42, 0.45), sky, None),
            (r(0.53, 0.6, 0.42, 0.2), bar, None),
        ]);
        let strip = Thumb::Paths(vec![
            (r(0.08, 0.1, 0.25, 0.32), sky, None),
            (r(0.33, 0.14, 0.6, 0.24), bar, None),
            (r(0.08, 0.55, 0.25, 0.32), sky, None),
            (r(0.33, 0.59, 0.6, 0.24), bar, None),
        ]);
        let sections = vec![Section {
            title: String::new(),
            cells: vec![
                Cell {
                    thumb: cards,
                    command: "r22.picLayout:picture".into(),
                    tip: self.r22_t("doc22.plCards", "Picture Caption Cards"),
                    checked: false,
                },
                Cell {
                    thumb: strip,
                    command: "r22.picLayout:pictureStrip".into(),
                    tip: self.r22_t("doc22.plStrip", "Picture Strip"),
                    checked: false,
                },
            ],
        }];
        self.r22_open("picture.layout", sections, Vec::new(), (3, (64.0, 56.0)))
    }

    /// Convert the selected picture into SmartArt `layout`: the picture becomes the first
    /// node's picture, with a caption placeholder (one undo step each: delete, insert).
    fn r22_picture_layout(&mut self, layout: &str) -> bool {
        use base64::Engine as _;
        self.menu = None;
        self.r22.gallery = None;
        let engine = self.view.engine();
        let Some(img) = engine.session().and_then(bo_edit::Session::tools_picture) else {
            return false;
        };
        let Some(bytes) = engine
            .document()
            .and_then(|d| d.media.get(&img.media).cloned())
        else {
            return false;
        };
        let mime = if bytes.starts_with(b"\x89PNG") {
            "image/png"
        } else if bytes.starts_with(&[0xFF, 0xD8]) {
            "image/jpeg"
        } else {
            return false;
        };
        let url = format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes.as_slice())
        );
        let width = (img.width.0 as f64 / 12_700.0).max(144.0);
        let caption = self.r22_t("doc22.plText", "[Text]");
        self.r3_run(vec![
            json!({ "cmd": "deleteBackward" }),
            json!({
                "cmd": "insertSmartArt",
                "layout": layout,
                "items": [{ "text": caption, "picture": url }],
                "width": width,
            }),
        ]);
        self.sync_context();
        true
    }

    /// WordArt Quick Styles as letters.
    fn r22_wordart(&mut self) -> bool {
        let cells = (0..ss::WORDART.len())
            .map(|i| Cell {
                thumb: wordart_thumb(i),
                command: format!("r20.wa:{i}"),
                tip: format!(
                    "{} {}",
                    self.r22_t("doc20.wordArtStyle", "WordArt Style"),
                    i + 1
                ),
                checked: false,
            })
            .collect();
        let sections = vec![Section {
            title: String::new(),
            cells,
        }];
        self.r22_open("shape.wordArt", sections, Vec::new(), (5, (66.0, 66.0)))
    }

    /// Picture Styles (Quick Styles) as framed pictures.
    fn r22_picture_styles(&mut self) -> bool {
        let cells = ps::QUICK_STYLES
            .iter()
            .enumerate()
            .map(|(i, q)| {
                let (border, bw, shadow, refl, soft, bevel, rot, shape) = *q;
                Cell {
                    thumb: effect_thumb(Fx {
                        picture: true,
                        shadow,
                        reflection: refl,
                        soft,
                        bevel,
                        rotation: rot,
                        border: border.map(|c| (c, bw)),
                        shape,
                        glow: None,
                    }),
                    command: format!("r19.quick:{i}"),
                    tip: format!(
                        "{} {}",
                        self.r22_t("doc19.quickStyle", "Picture Style"),
                        i + 1
                    ),
                    checked: false,
                }
            })
            .collect();
        let sections = vec![Section {
            title: String::new(),
            cells,
        }];
        self.r22_open(
            "picture.quickStyles",
            sections,
            Vec::new(),
            (6, (60.0, 56.0)),
        )
    }

    /// One category of Picture Effects (`prefix` `r19.fx`) or Shape Effects (`r20.fx`) as
    /// thumbnails: the "No …" cell, then the presets.
    #[allow(clippy::too_many_lines)]
    fn r22_effects(&mut self, owner: &str, prefix: &str, kind: &str, picture: bool) -> bool {
        let base = Fx {
            picture,
            ..Fx::default()
        };
        let (none_key, none_en) = match kind {
            "preset" => ("doc19.noPreset", "No Presets"),
            "shadow" => ("doc19.noShadow", "No Shadow"),
            "reflection" => ("doc19.noReflection", "No Reflection"),
            "glow" => ("doc19.noGlow", "No Glow"),
            "softEdge" => ("doc19.noSoftEdges", "No Soft Edges"),
            "bevel" => ("doc19.noBevel", "No Bevel"),
            _ => ("doc19.noRotation", "No Rotation"),
        };
        let none = self.r22_t(none_key, none_en);
        // The current effects of the picture or shape (the matching cell is framed).
        let session = self.view.engine().session();
        let fx = if picture {
            session
                .and_then(bo_edit::Session::tools_picture)
                .and_then(|i| i.format.and_then(|f| f.effects.map(|e| *e)))
        } else {
            session
                .and_then(bo_edit::Session::tools_shape)
                .and_then(|i| i.shape)
                .and_then(|s| s.style)
                .and_then(|st| st.effects)
        }
        .unwrap_or_default();
        let on = |preset: &str| -> bool {
            match kind {
                "glow" => fx.glow.as_ref().is_some_and(|g| {
                    preset.split_once(':').is_some_and(|(sz, c)| {
                        format!("{:06X}", g.color) == c
                            && sz
                                .parse::<f64>()
                                .is_ok_and(|v| (g.rad as f64 / 12_700.0 - v).abs() < 0.01)
                    })
                }),
                "softEdge" => fx.soft_edge.is_some_and(|e| {
                    preset
                        .parse::<f64>()
                        .is_ok_and(|v| (e as f64 / 12_700.0 - v).abs() < 0.01)
                }),
                "bevel" => fx.bevel.as_ref().is_some_and(|b| b.prst == preset),
                "rotation" => fx.camera.as_deref() == Some(preset),
                _ => false,
            }
        };
        let cmd = |preset: &str| format!("{prefix}:{kind}={preset}");
        let mut sections = vec![Section {
            title: none.clone(),
            cells: vec![Cell {
                thumb: Thumb::None,
                command: cmd("none"),
                tip: none,
                checked: false,
            }],
        }];
        let cell = |thumb: Thumb, preset: String, tip: String| Cell {
            thumb,
            command: cmd(&preset),
            tip,
            checked: on(&preset),
        };
        let title = |k: &str, en: &str| self.r22_t(k, en);
        match kind {
            "preset" => sections.push(Section {
                title: title("doc22.presets", "Presets"),
                cells: ps::PRESETS
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let (sh, re, gl, so, be, ro) = *p;
                        let glow = gl.and_then(|g| {
                            let (s, c) = g.split_once(':')?;
                            Some((s.parse().ok()?, u32::from_str_radix(c, 16).ok()?))
                        });
                        cell(
                            effect_thumb(Fx {
                                shadow: sh,
                                reflection: re,
                                glow,
                                soft: so,
                                bevel: be,
                                rotation: ro,
                                ..base
                            }),
                            i.to_string(),
                            format!("{} {}", title("doc19.kPreset", "Preset"), i + 1),
                        )
                    })
                    .collect(),
            }),
            "shadow" => {
                for (inner, k, en) in [
                    (false, "doc22.outer", "Outer"),
                    (true, "doc22.inner", "Inner"),
                ] {
                    sections.push(Section {
                        title: title(k, en),
                        cells: ps::SHADOWS
                            .iter()
                            .filter(|s| s.1 == inner)
                            .map(|s| {
                                cell(
                                    effect_thumb(Fx {
                                        shadow: Some(s.0),
                                        ..base
                                    }),
                                    s.0.to_owned(),
                                    words(s.0),
                                )
                            })
                            .collect(),
                    });
                }
            }
            "reflection" => sections.push(Section {
                title: title("doc22.reflectionVariations", "Reflection Variations"),
                cells: ps::REFLECTIONS
                    .iter()
                    .map(|r| {
                        cell(
                            effect_thumb(Fx {
                                reflection: Some(r.0),
                                ..base
                            }),
                            r.0.to_owned(),
                            words(r.0),
                        )
                    })
                    .collect(),
            }),
            "glow" => sections.push(Section {
                title: title("doc22.glowVariations", "Glow Variations"),
                cells: ps::GLOW_SIZES
                    .iter()
                    .flat_map(|s| ps::GLOW_COLORS.iter().map(move |c| (*s, *c)))
                    .map(|(s, c)| {
                        cell(
                            effect_thumb(Fx {
                                glow: Some((s * 4.0, c)),
                                ..base
                            }),
                            format!("{s}:{c:06X}"),
                            format!("{s} pt, #{c:06X}"),
                        )
                    })
                    .collect(),
            }),
            "softEdge" => sections.push(Section {
                title: title("doc22.softEdgeVariations", "Soft Edge Variations"),
                cells: ps::SOFT_EDGES
                    .iter()
                    .map(|r| {
                        cell(
                            effect_thumb(Fx {
                                soft: Some(*r),
                                ..base
                            }),
                            r.to_string(),
                            format!("{r} pt"),
                        )
                    })
                    .collect(),
            }),
            "bevel" => sections.push(Section {
                title: title("doc22.bevel", "Bevel"),
                cells: ps::BEVELS
                    .iter()
                    .map(|b| {
                        cell(
                            effect_thumb(Fx {
                                bevel: Some(*b),
                                ..base
                            }),
                            (*b).to_owned(),
                            words(b),
                        )
                    })
                    .collect(),
            }),
            _ => {
                for (fam, k, en) in [
                    ("isometric", "doc22.parallel", "Parallel"),
                    ("perspective", "doc22.perspective", "Perspective"),
                    ("oblique", "doc22.oblique", "Oblique"),
                ] {
                    sections.push(Section {
                        title: title(k, en),
                        cells: ps::ROTATIONS
                            .iter()
                            .filter(|r| r.starts_with(fam))
                            .map(|r| {
                                cell(
                                    effect_thumb(Fx {
                                        rotation: Some(*r),
                                        ..base
                                    }),
                                    (*r).to_owned(),
                                    words(r),
                                )
                            })
                            .collect(),
                    });
                }
            }
        }
        // The reference: three columns of 66 px cells (glow: six colours a row), then the
        // category's Options… row opening the Format pane at its section.
        let cols = if kind == "glow" { 6 } else { 3 };
        let (section, key, en) = match kind {
            "shadow" => ("shadow", "doc22.shadowOptions", "Shadow Options..."),
            "reflection" => (
                "reflection",
                "doc22.reflectionOptions",
                "Reflection Options...",
            ),
            "glow" => ("glow", "doc22.glowOptions", "Glow Options..."),
            "softEdge" => ("soft", "doc22.softOptions", "Soft Edges Options..."),
            "bevel" => ("format3d", "doc22.options3d", "3-D Options..."),
            "rotation" => (
                "rotation3d",
                "doc22.rotationOptions",
                "3-D Rotation Options...",
            ),
            _ => ("shadow", "doc22.options3d", "3-D Options..."),
        };
        let footer = vec![(self.r22_t(key, en), format!("r22.fxOptions:{section}"))];
        let cell = if kind == "glow" { 46.0 } else { 66.0 };
        self.r22_open(owner, sections, footer, (cols, (cell, cell)))
    }

    /// Input for the open gallery; `None` lets it through.
    pub(super) fn r22_input(&mut self, input: &Input) -> Option<bool> {
        let g = self.r22.gallery.as_mut()?;
        match *input {
            Input::Move { x, y } => Some(g.hover(x, y)),
            Input::Press { x, y, .. } => {
                if !g.contains(x, y) {
                    // A press outside closes the gallery (and is not passed on, as menus).
                    self.r22.gallery = None;
                    return Some(true);
                }
                let cmd = g.press(x, y);
                if let Some(cmd) = cmd {
                    self.r22.gallery = None;
                    self.action(&cmd);
                }
                Some(true)
            }
            Input::Key(crate::Key::Escape, _) => {
                self.r22.gallery = None;
                Some(true)
            }
            Input::Wheel(_) | Input::Release { .. } | Input::Drag { .. } => Some(false),
            _ => None,
        }
    }

    pub(super) fn r22_paint(&self, p: &mut dyn crate::Painter) {
        if let Some(g) = &self.r22.gallery {
            g.paint(p, &self.theme);
        }
    }
}

/// Labels and icons of the round-22 buttons (BlueOffice's own drawings).
pub(crate) fn install(icons: &mut crate::icons::Icons, s: &mut crate::strings::Strings) {
    let v = s.get("doc22.textDirection").to_owned();
    s.set(
        "commands.shape.textDirection",
        if v == "doc22.textDirection" {
            "Text Direction"
        } else {
            v.as_str()
        },
    );
    let rgb = |r, g, b| Some(Rgba(r, g, b, 255));
    // Text Direction: a letter column and a turning arrow.
    icons.add_solid(
        "shape.textDirection",
        &[
            ("M3 4h8v2h-3v12h-2v-12h-3Z", rgb(0x26, 0x26, 0x26)),
            (
                "M14 6h4a3 3 0 0 1 3 3v8h2l-3 4l-3-4h2v-8a1 1 0 0 0-1-1h-4Z",
                rgb(0x2f, 0x6f, 0xba),
            ),
        ],
    );
}
