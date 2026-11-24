//! Format Shape / Format Picture / Format Text Effects task pane of BlueDoc (round 21,
//! docs/ui/REPORT.md "Vòng 111"), docked at the right as the reference's (measured at
//! 125 %: pane 357 px; title, Shape Options / Text Options, the page icons Fill & Line,
//! Effects, Layout & Properties, Picture; collapsible sections with bold headings, labels
//! at the left, boxes right-aligned):
//!
//! * Fill & Line — Fill: No / Solid / Gradient / Picture or texture fill, Color,
//!   Transparency, gradient Type, Angle, stops (add, remove, Color, Position,
//!   Transparency); Line: No / Solid line, Color, Transparency, Width, Dash type;
//! * Effects — Shadow, Reflection, Glow (Presets ▾ and every setting), Soft Edges, 3-D
//!   Format (top bevel, width, height), 3-D Rotation (presets);
//! * Layout & Properties — Text Box: Vertical alignment, Left / Right / Top / Bottom
//!   margin;
//! * Picture — Picture Corrections (Sharpen/Soften, Brightness, Contrast), Picture Color
//!   (Saturation), Picture Transparency, Crop (left, top, right, bottom);
//! * Text Options — Text Fill (No / Solid), Text Outline (No / Solid, width), Text Effects
//!   (Shadow, Reflection, Glow presets).
//!
//! Every change builds one engine command (one undo step), run by the window
//! (`doc_r21`); the pane re-reads the selection afterwards.
use crate::spin_box::{paint_combo, paint_spin, spin_hit, BoxLook, SpinHit};
use crate::{Painter, TextStyle};
use bo_model::picture_style::PicEffects;
use bo_render::{PathSeg, Rgba};
use serde_json::{json, Value};
use std::collections::HashSet;

/// Pages (the icons under the tabs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Page {
    FillLine,
    Effects,
    Layout,
    Picture,
}

/// What the pane formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    Shape,
    Picture,
}

/// What a press asks the window to do.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PaneAct {
    None,
    Redraw,
    Close,
    /// Open the choices of control `id` under `anchor` (window px): (command id, label,
    /// checked) entries from [`FmtPane::menu`].
    Menu(&'static str, [f32; 4]),
    /// Run these engine commands (each one undo step) then re-read the selection.
    Run(Vec<Value>),
    /// Picture or texture fill › Insert…: ask the host for a picture.
    PickPicture,
}

/// Settings read from the selection.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Vals {
    /// 0 none, 1 solid, 2 gradient, 3 picture / texture.
    pub(crate) fill: u8,
    pub(crate) fill_color: u32,
    pub(crate) fill_transp: f64,
    /// 0 linear, 1 radial, 2 rectangular, 3 path.
    pub(crate) grad_kind: usize,
    pub(crate) grad_angle: f64,
    /// (position %, colour, transparency %).
    pub(crate) stops: Vec<(f64, u32, f64)>,
    pub(crate) stop: usize,
    /// Path gradients: fillToRect l, t, r, b (%).
    pub(crate) grad_focus: [f64; 4],
    /// 0 none, 1 solid.
    pub(crate) line: u8,
    pub(crate) line_color: u32,
    pub(crate) line_transp: f64,
    pub(crate) line_width: f64,
    pub(crate) dash: usize,
    pub(crate) shadow: Option<[f64; 6]>,
    pub(crate) shadow_color: u32,
    pub(crate) shadow_inner: bool,
    pub(crate) reflection: Option<[f64; 4]>,
    pub(crate) glow: Option<[f64; 2]>,
    pub(crate) glow_color: u32,
    pub(crate) soft: f64,
    pub(crate) bevel: Option<(String, f64, f64)>,
    pub(crate) camera: Option<String>,
    pub(crate) anchor: usize,
    /// Left, top, right, bottom (pt).
    pub(crate) insets: [f64; 4],
    /// Sharpen, brightness, contrast, saturation, transparency (%).
    pub(crate) fx: [f64; 5],
    pub(crate) crop: [f64; 4],
    pub(crate) text_fill: Option<u32>,
    pub(crate) text_line: Option<u32>,
    pub(crate) text_line_w: f64,
    /// The picture's colour adjustments as read (Recolor, grayscale… are kept).
    pub(crate) raw_fx: bo_model::picture::PictureFx,
}

impl Default for Vals {
    fn default() -> Self {
        Vals {
            fill: 1,
            fill_color: 0x44_72C4,
            fill_transp: 0.0,
            grad_kind: 0,
            grad_angle: 90.0,
            stops: vec![(0.0, 0x44_72C4, 0.0), (100.0, 0xFF_FFFF, 0.0)],
            stop: 0,
            grad_focus: [50.0; 4],
            line: 1,
            line_color: 0x2F_528F,
            line_transp: 0.0,
            line_width: 1.0,
            dash: 0,
            shadow: None,
            shadow_color: 0,
            shadow_inner: false,
            reflection: None,
            glow: None,
            glow_color: 0x44_72C4,
            soft: 0.0,
            bevel: None,
            camera: None,
            anchor: 0,
            insets: [7.2, 3.6, 7.2, 3.6],
            fx: [0.0, 0.0, 0.0, 100.0, 0.0],
            crop: [0.0; 4],
            text_fill: None,
            text_line: None,
            text_line_w: 0.75,
            raw_fx: bo_model::picture::PictureFx::default(),
        }
    }
}

const EMU_PT: f64 = 12_700.0;

impl Vals {
    /// The settings of a document image (a shape or a picture).
    #[allow(clippy::cast_precision_loss)]
    pub(crate) fn of(img: &bo_model::Image) -> Self {
        let mut v = Vals::default();
        let pt = |e: i64| e as f64 / EMU_PT;
        let pc = |a: i32| f64::from(a) / 1000.0;
        let mut effects: Option<&PicEffects> = None;
        if let Some(sh) = img.shape.as_deref() {
            let st = sh.style.as_deref();
            match &sh.fill {
                Some(c) => {
                    v.fill = 1;
                    v.fill_color = rgb_of(*c);
                }
                None => v.fill = 0,
            }
            if let Some(s) = st {
                if let Some(g) = &s.gradient {
                    v.fill = 2;
                    v.grad_kind = ["linear", "circle", "rect", "shape"]
                        .iter()
                        .position(|k| *k == g.kind)
                        .unwrap_or(0);
                    v.grad_angle = f64::from(g.angle) / 60_000.0;
                    v.grad_focus = g.focus.map(|f| f64::from(f) / 1000.0);
                    v.stops = g
                        .stops
                        .iter()
                        .map(|s| (pc(s.pos), s.color, 100.0 - pc(s.alpha)))
                        .collect();
                }
                if s.picture.is_some() {
                    v.fill = 3;
                }
                v.fill_transp = s.fill_alpha.map_or(0.0, |a| 100.0 - pc(a));
                v.line_transp = s.line_alpha.map_or(0.0, |a| 100.0 - pc(a));
                effects = s.effects.as_ref();
            }
            match &sh.line {
                Some(l) => {
                    v.line = 1;
                    v.line_color = rgb_of(l.color);
                    v.line_width = pt(l.width.0);
                    v.dash = DASHES
                        .iter()
                        .position(|d| Some(*d) == l.dash.as_deref())
                        .unwrap_or(0);
                }
                None => v.line = 0,
            }
            v.anchor = match sh.anchor {
                bo_model::TextAnchor::Top => 0,
                bo_model::TextAnchor::Center => 1,
                bo_model::TextAnchor::Bottom => 2,
            };
            let ins = sh.insets;
            v.insets = [pt(ins[0].0), pt(ins[1].0), pt(ins[2].0), pt(ins[3].0)];
            v.text_fill_from(sh);
        }
        if let Some(f) = img.format.as_deref() {
            if img.shape.is_none() {
                match &f.line {
                    Some(l) => {
                        v.line = 1;
                        v.line_color = l.color;
                        v.line_width = pt(l.width);
                        v.dash = DASHES
                            .iter()
                            .position(|d| Some(*d) == l.dash.as_deref())
                            .unwrap_or(0);
                    }
                    None => v.line = 0,
                }
                v.fill = 0;
                effects = f.effects.as_deref();
            }
            let fx = &f.fx;
            v.raw_fx = fx.clone();
            v.fx = [
                pc(fx.sharpen),
                pc(fx.bright),
                pc(fx.contrast),
                fx.saturation.map_or(100.0, pc),
                fx.alpha.map_or(0.0, |a| 100.0 - pc(a)),
            ];
        }
        if let Some(c) = img.crop {
            v.crop = [pc(c[0]), pc(c[1]), pc(c[2]), pc(c[3])];
        }
        if let Some(e) = effects {
            if let Some(s) = &e.shadow {
                v.shadow = Some([
                    100.0 - pc(s.alpha),
                    if s.inner { 100.0 } else { pc(s.sx) },
                    pt(s.blur),
                    f64::from(s.dir) / 60_000.0,
                    pt(s.dist),
                    0.0,
                ]);
                v.shadow_color = s.color;
                v.shadow_inner = s.inner;
            }
            if let Some(r) = &e.reflection {
                v.reflection = Some([100.0 - pc(r.st_a), pc(r.end_pos), pt(r.blur), pt(r.dist)]);
            }
            if let Some(g) = &e.glow {
                v.glow = Some([pt(g.rad), 100.0 - pc(g.alpha)]);
                v.glow_color = g.color;
            }
            v.soft = e.soft_edge.map_or(0.0, pt);
            v.bevel = e.bevel.as_ref().map(|b| (b.prst.clone(), pt(b.w), pt(b.h)));
            v.camera.clone_from(&e.camera);
        }
        v
    }

    fn text_fill_from(&mut self, sh: &bo_model::Shape) {
        let Some(blocks) = &sh.text else {
            return;
        };
        for b in blocks {
            if let bo_model::Block::Paragraph(p) = b {
                if let Some(r) = p.runs.first() {
                    if let Some(e) = r.props.effects.as_ref() {
                        if let Some(f) = &e.fill {
                            if let bo_model::run_fx::FxFill::Solid { color } = &f.fill {
                                self.text_fill = Some(rgb_of(color.rgb));
                            }
                        }
                        if let Some(o) = &e.outline {
                            if let bo_model::run_fx::FxFill::Solid { color } = &o.fill {
                                self.text_line = Some(rgb_of(color.rgb));
                            }
                            #[allow(clippy::cast_precision_loss)]
                            let w = o.width as f64 / EMU_PT;
                            self.text_line_w = w;
                        }
                    }
                    return;
                }
            }
        }
    }
}

/// Dash types offered (§20.1.10.48 ST_PresetLineDashVal; index 0 = solid).
pub(crate) const DASHES: [&str; 8] = [
    "solid",
    "sysDot",
    "sysDash",
    "dash",
    "dashDot",
    "lgDash",
    "lgDashDot",
    "lgDashDotDot",
];
/// Gradient types (label key, English).
const GRAD_KINDS: [(&str, &str); 4] = [
    ("doc21.fpLinear", "Linear"),
    ("doc21.fpRadial", "Radial"),
    ("doc21.fpRectangular", "Rectangular"),
    ("doc21.fpPath", "Path"),
];
/// Vertical alignment choices (anchor value, label key, English).
const ANCHORS: [(&str, &str, &str); 3] = [
    ("t", "doc21.fpTop", "Top"),
    ("ctr", "doc21.fpMiddle", "Middle"),
    ("b", "doc21.fpBottom", "Bottom"),
];

/// One laid-out row of the pane.
#[derive(Debug, Clone, PartialEq)]
struct Row {
    /// Top (logical px from the content top) and height.
    y: f32,
    h: f32,
    kind: RowKind,
}

#[derive(Debug, Clone, PartialEq)]
enum RowKind {
    Section(&'static str, &'static str, &'static str),
    Radio(&'static str, u8, &'static str, &'static str),
    Color(&'static str, &'static str, &'static str),
    Spin(&'static str, &'static str, &'static str),
    Choice(&'static str, &'static str, &'static str),
    Stops,
    /// Pattern fill: the 48 preset patterns.
    Patterns,
    Button(&'static str, &'static str, &'static str),
}

/// The open pane.
pub(crate) struct FmtPane {
    pub(crate) target: Target,
    /// Text Options tab (shapes).
    pub(crate) text_tab: bool,
    pub(crate) page: Page,
    open: HashSet<&'static str>,
    pub(crate) vals: Vals,
    /// Window rectangle of the pane (device px) and scale.
    rect: [f32; 4],
    scale: f32,
    scroll: f32,
    hover: Option<(f32, f32)>,
    /// Pattern fill chosen here (preset, foreground, background): the document keeps it as a
    /// tiled picture.
    pattern: Option<(usize, u32, u32)>,
    /// Brightness of a gradient stop: (stop, colour it started from, %).
    bright: (usize, u32, f64),
    /// Size ▸ in the title menu: the wider pane.
    wide: bool,
}

/// Pane width (the reference's 357 px at 125 %).
pub(crate) fn width(scale: f32) -> f32 {
    (286.0 * scale).round()
}

impl FmtPane {
    /// A pane for `target` on `page` with `section` expanded.
    pub(crate) fn new(target: Target, page: Page, section: Option<&'static str>) -> Self {
        let mut open = HashSet::new();
        if let Some(s) = section {
            open.insert(s);
        }
        FmtPane {
            target,
            text_tab: false,
            page,
            open,
            vals: Vals::default(),
            rect: [0.0; 4],
            scale: 1.0,
            scroll: 0.0,
            hover: None,
            pattern: None,
            bright: (usize::MAX, 0, 0.0),
            wide: false,
        }
    }

    /// Width of this pane (window px).
    pub(crate) fn pane_width(&self, scale: f32) -> f32 {
        if self.wide {
            (360.0 * scale).round()
        } else {
            width(scale)
        }
    }

    /// Title menu › Size: switch between the reference's width and a wider pane.
    pub(crate) fn toggle_size(&mut self) {
        self.wide = !self.wide;
    }

    /// The fill option shown (a pattern is a tiled picture in the document).
    fn fill_kind(&self) -> u8 {
        if self.pattern.is_some() && self.vals.fill == 3 {
            4
        } else {
            self.vals.fill
        }
    }

    /// The pattern fill as a tiled two-colour picture.
    fn pattern_cmd(&self) -> Value {
        use base64::Engine as _;
        let (i, fg, bg) = self.pattern.unwrap_or((0, 0x44_72C4, 0xFF_FFFF));
        let name = crate::slide_bgpane::PATTERNS[i.min(crate::slide_bgpane::PATTERNS.len() - 1)];
        let bmp = pattern_bmp(crate::slide_bgpane::pattern_bits(name), fg, bg);
        json!({
            "cmd": "shapePictureFill",
            "data": base64::engine::general_purpose::STANDARD.encode(bmp),
            "tile": true,
        })
    }

    /// Choices of drop-down `id` with their labels: (choice, label, checked).
    pub(crate) fn menu_labels(
        &self,
        id: &str,
        t: &dyn Fn(&str, &str) -> String,
    ) -> Vec<(String, String, bool)> {
        const DIRS: [(&str, &str, f64, [f64; 4]); 9] = [
            (
                "doc20.dTlBr",
                "Diagonal, top left to bottom right",
                45.0,
                [0.0, 0.0, 100.0, 100.0],
            ),
            ("doc20.dDown", "Downwards", 90.0, [50.0, 0.0, 50.0, 100.0]),
            (
                "doc20.dTrBl",
                "Diagonal, top right to bottom left",
                135.0,
                [100.0, 0.0, 0.0, 100.0],
            ),
            (
                "doc20.dRight",
                "To the right",
                0.0,
                [0.0, 50.0, 100.0, 50.0],
            ),
            (
                "doc20.dCenter",
                "Out from the centre",
                -1.0,
                [50.0, 50.0, 50.0, 50.0],
            ),
            (
                "doc20.dLeft",
                "To the left",
                180.0,
                [100.0, 50.0, 0.0, 50.0],
            ),
            (
                "doc20.dBlTr",
                "Diagonal, bottom left to top right",
                315.0,
                [0.0, 100.0, 100.0, 0.0],
            ),
            ("doc20.dUp", "Upwards", 270.0, [50.0, 100.0, 50.0, 0.0]),
            (
                "doc20.dBrTl",
                "Diagonal, bottom right to top left",
                225.0,
                [100.0, 100.0, 0.0, 0.0],
            ),
        ];
        match id {
            "gradPreset" => {
                let mut out = Vec::new();
                for (var, k, en) in [
                    ("light", "doc20.light", "Light"),
                    ("dark", "doc20.dark", "Dark"),
                ] {
                    for (i, d) in DIRS.iter().enumerate() {
                        out.push((
                            format!("fp21.gv:{var}:{i}"),
                            format!("{}: {}", t(k, en), t(d.0, d.1)),
                            false,
                        ));
                    }
                }
                out
            }
            "gradDir" => DIRS
                .iter()
                .filter(|d| self.vals.grad_kind != 0 || d.2 >= 0.0)
                .map(|d| {
                    let choice = if self.vals.grad_kind == 0 {
                        format!("fp21.ga:{}", d.2)
                    } else {
                        format!("fp21.gf:{},{},{},{}", d.3[0], d.3[1], d.3[2], d.3[3])
                    };
                    (choice, t(d.0, d.1), false)
                })
                .collect(),
            "paneMenu" => vec![
                (
                    "fp21.paneSize".to_owned(),
                    t("doc21.fpPaneSize", "Size"),
                    self.wide,
                ),
                (
                    "fp21.paneClose".to_owned(),
                    t("doc21.fpPaneClose", "Close"),
                    false,
                ),
            ],
            _ => self
                .menu(id)
                .into_iter()
                .map(|(c, k, en, on)| (c, if k.is_empty() { en } else { t(&k, &en) }, on))
                .collect(),
        }
    }

    pub(crate) fn layout(&mut self, rect: [f32; 4], scale: f32) {
        self.rect = rect;
        self.scale = scale;
    }

    pub(crate) fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.rect[0]
            && x < self.rect[0] + self.rect[2]
            && y >= self.rect[1]
            && y < self.rect[1] + self.rect[3]
    }

    /// The pages shown as icons.
    fn pages(&self) -> Vec<Page> {
        if self.text_tab {
            return vec![Page::FillLine, Page::Effects, Page::Layout];
        }
        match self.target {
            Target::Shape => vec![Page::FillLine, Page::Effects, Page::Layout],
            Target::Picture => vec![Page::FillLine, Page::Effects, Page::Picture],
        }
    }

    /// Rows of the current page.
    #[allow(clippy::too_many_lines)]
    fn rows(&self) -> Vec<Row> {
        use RowKind::{Button, Choice, Color, Patterns, Radio, Section, Spin, Stops};
        let mut out = Vec::new();
        let mut y = 0.0;
        let v = &self.vals;
        let mut push = |kind: RowKind, h: f32| {
            // Row pitch measured on the reference at 125 %: option buttons 36 px, boxes
            // 43 px (29 / 34 logical px).
            let h = match kind {
                Radio(..) => 29.0,
                Spin(..) | Color(..) | Choice(..) => 34.0,
                _ => h,
            };
            out.push(Row { y, h, kind });
            y += h;
        };
        let sec = |id: &'static str, k: &'static str, en: &'static str| Section(id, k, en);
        let open = |id: &str| self.open.contains(id);
        if self.text_tab {
            match self.page {
                Page::FillLine => {
                    push(sec("tfill", "doc21.fpTextFill", "Text Fill"), 31.0);
                    if open("tfill") {
                        push(Radio("tfill", 0, "doc21.fpNoFill", "No fill"), 24.0);
                        push(Radio("tfill", 1, "doc21.fpSolidFill", "Solid fill"), 24.0);
                        if v.text_fill.is_some() {
                            push(Color("tfillColor", "doc21.fpColor", "Color"), 31.0);
                        }
                    }
                    push(sec("tline", "doc21.fpTextOutline", "Text Outline"), 31.0);
                    if open("tline") {
                        push(Radio("tline", 0, "doc21.fpNoLine", "No line"), 24.0);
                        push(Radio("tline", 1, "doc21.fpSolidLine", "Solid line"), 24.0);
                        if v.text_line.is_some() {
                            push(Color("tlineColor", "doc21.fpColor", "Color"), 31.0);
                            push(Spin("tlineW", "doc21.fpWidth", "Width"), 31.0);
                        }
                    }
                }
                Page::Effects => {
                    for (id, k, en) in [
                        ("tshadow", "doc21.fpShadow", "Shadow"),
                        ("treflection", "doc21.fpReflection", "Reflection"),
                        ("tglow", "doc21.fpGlow", "Glow"),
                    ] {
                        push(sec(id, k, en), 31.0);
                        if open(id) {
                            push(Choice(id, "doc21.fpPresets", "Presets"), 31.0);
                        }
                    }
                }
                _ => Self::layout_rows(&mut push, &open),
            }
            return out;
        }
        match self.page {
            Page::FillLine => {
                push(sec("fill", "doc21.fpFill", "Fill"), 31.0);
                if open("fill") {
                    if self.target == Target::Shape {
                        for (i, k, en) in [
                            (0, "doc21.fpNoFill", "No fill"),
                            (1, "doc21.fpSolidFill", "Solid fill"),
                            (2, "doc21.fpGradientFill", "Gradient fill"),
                            (3, "doc21.fpPictureFill", "Picture or texture fill"),
                            (4, "doc21.fpPatternFill", "Pattern fill"),
                        ] {
                            push(Radio("fill", i, k, en), 24.0);
                        }
                        match self.fill_kind() {
                            1 => {
                                push(Color("fillColor", "doc21.fpColor", "Color"), 31.0);
                                push(
                                    Spin("fillTransp", "doc21.fpTransparency", "Transparency"),
                                    29.0,
                                );
                            }
                            2 => {
                                push(
                                    Choice(
                                        "gradPreset",
                                        "doc21.fpPresetGradients",
                                        "Preset gradients",
                                    ),
                                    28.0,
                                );
                                push(Choice("gradKind", "doc21.fpType", "Type"), 28.0);
                                push(Choice("gradDir", "doc21.fpDirection", "Direction"), 28.0);
                                // Greyed for the other types, as the reference.
                                push(Spin("gradAngle", "doc21.fpAngle", "Angle"), 29.0);
                                push(Stops, 58.0);
                                push(Color("stopColor", "doc21.fpColor", "Color"), 31.0);
                                push(Spin("stopPos", "doc21.fpPosition", "Position"), 29.0);
                                push(
                                    Spin("stopTransp", "doc21.fpTransparency", "Transparency"),
                                    29.0,
                                );
                                push(Spin("stopBright", "doc21.fpBrightness", "Brightness"), 29.0);
                            }
                            4 => {
                                push(Patterns, 142.0);
                                push(Color("patFg", "doc21.fpForeground", "Foreground"), 31.0);
                                push(Color("patBg", "doc21.fpBackground", "Background"), 31.0);
                            }
                            3 => {
                                push(Button("picInsert", "doc21.fpInsert", "Insert..."), 34.0);
                                push(Choice("texture", "doc21.fpTexture", "Texture"), 31.0);
                            }
                            _ => {}
                        }
                    } else {
                        push(Radio("fill", 0, "doc21.fpNoFill", "No fill"), 24.0);
                    }
                }
                push(sec("line", "doc21.fpLine", "Line"), 31.0);
                if open("line") {
                    push(Radio("line", 0, "doc21.fpNoLine", "No line"), 24.0);
                    push(Radio("line", 1, "doc21.fpSolidLine", "Solid line"), 24.0);
                    if v.line == 1 {
                        push(Color("lineColor", "doc21.fpColor", "Color"), 31.0);
                        if self.target == Target::Shape {
                            push(
                                Spin("lineTransp", "doc21.fpTransparency", "Transparency"),
                                29.0,
                            );
                        }
                        push(Spin("lineWidth", "doc21.fpWidth", "Width"), 29.0);
                        push(Choice("dash", "doc21.fpDash", "Dash type"), 31.0);
                    }
                }
            }
            Page::Effects => {
                push(sec("shadow", "doc21.fpShadow", "Shadow"), 31.0);
                if open("shadow") {
                    push(Choice("shadowPresets", "doc21.fpPresets", "Presets"), 31.0);
                    // The settings show (empty) before an effect is chosen, as the reference.
                    push(Color("shadowColor", "doc21.fpColor", "Color"), 31.0);
                    for (id, k, en) in [
                        ("shTransp", "doc21.fpTransparency", "Transparency"),
                        ("shSize", "doc21.fpSize", "Size"),
                        ("shBlur", "doc21.fpBlur", "Blur"),
                        ("shAngle", "doc21.fpAngle", "Angle"),
                        ("shDist", "doc21.fpDistance", "Distance"),
                    ] {
                        push(Spin(id, k, en), 29.0);
                    }
                }
                push(sec("reflection", "doc21.fpReflection", "Reflection"), 31.0);
                if open("reflection") {
                    push(Choice("reflPresets", "doc21.fpPresets", "Presets"), 31.0);
                    for (id, k, en) in [
                        ("rfTransp", "doc21.fpTransparency", "Transparency"),
                        ("rfSize", "doc21.fpSize", "Size"),
                        ("rfBlur", "doc21.fpBlur", "Blur"),
                        ("rfDist", "doc21.fpDistance", "Distance"),
                    ] {
                        push(Spin(id, k, en), 29.0);
                    }
                }
                push(sec("glow", "doc21.fpGlow", "Glow"), 31.0);
                if open("glow") {
                    push(Choice("glowPresets", "doc21.fpPresets", "Presets"), 31.0);
                    push(Color("glowColor", "doc21.fpColor", "Color"), 31.0);
                    push(Spin("glSize", "doc21.fpSize", "Size"), 29.0);
                    push(
                        Spin("glTransp", "doc21.fpTransparency", "Transparency"),
                        29.0,
                    );
                }
                push(sec("soft", "doc21.fpSoftEdges", "Soft Edges"), 31.0);
                if open("soft") {
                    push(Choice("softPresets", "doc21.fpPresets", "Presets"), 31.0);
                    push(Spin("softSize", "doc21.fpSize", "Size"), 29.0);
                }
                push(sec("format3d", "doc21.fp3dFormat", "3-D Format"), 31.0);
                if open("format3d") {
                    push(Choice("bevel", "doc21.fpTopBevel", "Top bevel"), 31.0);
                    push(Spin("bevelW", "doc21.fpWidth", "Width"), 29.0);
                    push(Spin("bevelH", "doc21.fpHeight", "Height"), 29.0);
                }
                push(
                    sec("rotation3d", "doc21.fp3dRotation", "3-D Rotation"),
                    31.0,
                );
                if open("rotation3d") {
                    push(Choice("rotation", "doc21.fpPresets", "Presets"), 31.0);
                }
            }
            Page::Layout => Self::layout_rows(&mut push, &open),
            Page::Picture => {
                push(
                    sec("corr", "doc21.fpCorrections", "Picture Corrections"),
                    31.0,
                );
                if open("corr") {
                    push(Spin("fxSharp", "doc21.fpSharpness", "Sharpness"), 29.0);
                    push(Spin("fxBright", "doc21.fpBrightness", "Brightness"), 29.0);
                    push(Spin("fxContrast", "doc21.fpContrast", "Contrast"), 29.0);
                    push(Button("fxReset", "doc21.fpReset", "Reset"), 34.0);
                }
                push(sec("color", "doc21.fpPictureColor", "Picture Color"), 31.0);
                if open("color") {
                    push(Spin("fxSat", "doc21.fpSaturation", "Saturation"), 29.0);
                }
                push(
                    sec(
                        "transp",
                        "doc21.fpPictureTransparency",
                        "Picture Transparency",
                    ),
                    31.0,
                );
                if open("transp") {
                    push(
                        Spin("fxTransp", "doc21.fpTransparency", "Transparency"),
                        29.0,
                    );
                }
                push(sec("crop", "doc21.fpCrop", "Crop"), 31.0);
                if open("crop") {
                    for (id, k, en) in [
                        ("cropL", "doc21.fpLeft", "Left"),
                        ("cropT", "doc21.fpTopCrop", "Top"),
                        ("cropR", "doc21.fpRight", "Right"),
                        ("cropB", "doc21.fpBottomCrop", "Bottom"),
                    ] {
                        push(Spin(id, k, en), 29.0);
                    }
                }
            }
        }
        out
    }

    fn layout_rows(push: &mut dyn FnMut(RowKind, f32), open: &dyn Fn(&str) -> bool) {
        use RowKind::{Choice, Section, Spin};
        push(Section("textbox", "doc21.fpTextBox", "Text Box"), 31.0);
        if open("textbox") {
            push(
                Choice("anchor", "doc21.fpVertical", "Vertical alignment"),
                31.0,
            );
            for (id, k, en) in [
                ("insL", "doc21.fpLeftMargin", "Left margin"),
                ("insR", "doc21.fpRightMargin", "Right margin"),
                ("insT", "doc21.fpTopMargin", "Top margin"),
                ("insB", "doc21.fpBottomMargin", "Bottom margin"),
            ] {
                push(Spin(id, k, en), 31.0);
            }
        }
    }

    /// The value shown in spin box `id` and its step, range and unit.
    fn spin(&self, id: &str) -> (f64, f64, f64, f64, &'static str) {
        let v = &self.vals;
        let sh = v.shadow.unwrap_or([60.0, 100.0, 4.0, 45.0, 3.0, 0.0]);
        let rf = v.reflection.unwrap_or([48.0, 55.0, 0.5, 0.0]);
        let gl = v.glow.unwrap_or([8.0, 60.0]);
        let stop = v.stops.get(v.stop).copied().unwrap_or((0.0, 0, 0.0));
        let bv = v.bevel.clone().unwrap_or(("circle".into(), 6.0, 6.0));
        match id {
            "fillTransp" => (v.fill_transp, 1.0, 0.0, 100.0, "%"),
            "lineTransp" => (v.line_transp, 1.0, 0.0, 100.0, "%"),
            "lineWidth" => (v.line_width, 0.25, 0.0, 1584.0, " pt"),
            "gradAngle" => (v.grad_angle, 1.0, 0.0, 359.9, "\u{B0}"),
            "stopPos" => (stop.0, 1.0, 0.0, 100.0, "%"),
            "stopTransp" => (stop.2, 1.0, 0.0, 100.0, "%"),
            "stopBright" => (
                if self.bright.0 == v.stop {
                    self.bright.2
                } else {
                    0.0
                },
                1.0,
                -100.0,
                100.0,
                "%",
            ),
            "shTransp" => (sh[0], 1.0, 0.0, 100.0, "%"),
            "shSize" => (sh[1], 1.0, 1.0, 200.0, "%"),
            "shBlur" => (sh[2], 1.0, 0.0, 100.0, " pt"),
            "shAngle" => (sh[3], 1.0, 0.0, 359.9, "\u{B0}"),
            "shDist" => (sh[4], 1.0, 0.0, 200.0, " pt"),
            "rfTransp" => (rf[0], 1.0, 0.0, 100.0, "%"),
            "rfSize" => (rf[1], 1.0, 0.0, 100.0, "%"),
            "rfBlur" => (rf[2], 0.5, 0.0, 100.0, " pt"),
            "rfDist" => (rf[3], 1.0, 0.0, 100.0, " pt"),
            "glSize" => (gl[0], 1.0, 0.0, 150.0, " pt"),
            "glTransp" => (gl[1], 1.0, 0.0, 100.0, "%"),
            "softSize" => (v.soft, 1.0, 0.0, 100.0, " pt"),
            "bevelW" => (bv.1, 1.0, 0.0, 1584.0, " pt"),
            "bevelH" => (bv.2, 1.0, 0.0, 1584.0, " pt"),
            "insL" => (v.insets[0], 0.72, 0.0, 1584.0, " pt"),
            "insT" => (v.insets[1], 0.72, 0.0, 1584.0, " pt"),
            "insR" => (v.insets[2], 0.72, 0.0, 1584.0, " pt"),
            "insB" => (v.insets[3], 0.72, 0.0, 1584.0, " pt"),
            "fxSharp" => (v.fx[0], 1.0, -100.0, 100.0, "%"),
            "fxBright" => (v.fx[1], 1.0, -100.0, 100.0, "%"),
            "fxContrast" => (v.fx[2], 1.0, -100.0, 100.0, "%"),
            "fxSat" => (v.fx[3], 1.0, 0.0, 400.0, "%"),
            "fxTransp" => (v.fx[4], 1.0, 0.0, 100.0, "%"),
            "cropL" => (v.crop[0], 1.0, 0.0, 99.0, "%"),
            "cropT" => (v.crop[1], 1.0, 0.0, 99.0, "%"),
            "cropR" => (v.crop[2], 1.0, 0.0, 99.0, "%"),
            "cropB" => (v.crop[3], 1.0, 0.0, 99.0, "%"),
            "tlineW" => (v.text_line_w, 0.25, 0.0, 1584.0, " pt"),
            _ => (0.0, 1.0, 0.0, 100.0, ""),
        }
    }

    /// Set spin `id` to `x` and the commands that make it so.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn set_spin(&mut self, id: &str, x: f64) -> Vec<Value> {
        let (_, _, lo, hi, _) = self.spin(id);
        let x = (x.clamp(lo, hi) * 100.0).round() / 100.0;
        let shape = self.target == Target::Shape;
        if id == "stopBright" {
            let stop = self.vals.stop;
            let current = self.vals.stops.get(stop).map_or(0, |s| s.1);
            let base = if self.bright.0 == stop {
                self.bright.1
            } else {
                current
            };
            self.bright = (stop, base, x);
            if let Some(s) = self.vals.stops.get_mut(stop) {
                s.1 = brighten(base, x);
            }
            return vec![self.gradient_cmd()];
        }
        let effect = |kind: &str, preset: String| json!({ "cmd": if shape { "shapeEffect" } else { "pictureEffect" }, "kind": kind, "preset": preset });
        let v = &mut self.vals;
        match id {
            "fillTransp" => {
                v.fill_transp = x;
                vec![json!({ "cmd": "shapeTransparency", "fill": x })]
            }
            "lineTransp" => {
                v.line_transp = x;
                vec![json!({ "cmd": "shapeTransparency", "line": x })]
            }
            "lineWidth" => {
                v.line_width = x.max(0.25);
                vec![if shape {
                    json!({ "cmd": "pictureShapeOutline", "weight": v.line_width })
                } else {
                    json!({ "cmd": "pictureBorder", "weight": v.line_width })
                }]
            }
            "gradAngle" => {
                v.grad_angle = x;
                vec![self.gradient_cmd()]
            }
            "stopPos" | "stopTransp" => {
                if let Some(s) = v.stops.get_mut(v.stop) {
                    if id == "stopPos" {
                        s.0 = x;
                    } else {
                        s.2 = x;
                    }
                }
                vec![self.gradient_cmd()]
            }
            "shTransp" | "shSize" | "shBlur" | "shAngle" | "shDist" => {
                let i = ["shTransp", "shSize", "shBlur", "shAngle", "shDist"]
                    .iter()
                    .position(|k| *k == id)
                    .unwrap_or(0);
                let mut s = v.shadow.unwrap_or([60.0, 100.0, 4.0, 45.0, 3.0, 0.0]);
                s[i] = x;
                v.shadow = Some(s);
                vec![effect("shadow", self.shadow_preset())]
            }
            "rfTransp" | "rfSize" | "rfBlur" | "rfDist" => {
                let i = ["rfTransp", "rfSize", "rfBlur", "rfDist"]
                    .iter()
                    .position(|k| *k == id)
                    .unwrap_or(0);
                let mut r = v.reflection.unwrap_or([48.0, 55.0, 0.5, 0.0]);
                r[i] = x;
                v.reflection = Some(r);
                vec![effect("reflection", self.reflection_preset())]
            }
            "glSize" | "glTransp" => {
                let mut g = v.glow.unwrap_or([8.0, 60.0]);
                g[usize::from(id == "glTransp")] = x;
                v.glow = Some(g);
                vec![effect("glow", self.glow_preset())]
            }
            "softSize" => {
                v.soft = x;
                vec![effect(
                    "softEdge",
                    if x <= 0.0 { "none".into() } else { fmt(x) },
                )]
            }
            "bevelW" | "bevelH" => {
                let mut b = v.bevel.clone().unwrap_or(("circle".into(), 6.0, 6.0));
                if id == "bevelW" {
                    b.1 = x;
                } else {
                    b.2 = x;
                }
                let preset = format!("custom:type={};width={};height={}", b.0, fmt(b.1), fmt(b.2));
                v.bevel = Some(b);
                vec![effect("bevel", preset)]
            }
            "insL" | "insT" | "insR" | "insB" => {
                let i = ["insL", "insT", "insR", "insB"]
                    .iter()
                    .position(|k| *k == id)
                    .unwrap_or(0);
                v.insets[i] = x;
                vec![json!({ "cmd": "shapeTextInsets", "insets": v.insets })]
            }
            "fxSharp" | "fxBright" | "fxContrast" | "fxSat" | "fxTransp" => {
                let i = ["fxSharp", "fxBright", "fxContrast", "fxSat", "fxTransp"]
                    .iter()
                    .position(|k| *k == id)
                    .unwrap_or(0);
                v.fx[i] = x;
                vec![self.fx_cmd()]
            }
            "cropL" | "cropT" | "cropR" | "cropB" => {
                let i = ["cropL", "cropT", "cropR", "cropB"]
                    .iter()
                    .position(|k| *k == id)
                    .unwrap_or(0);
                v.crop[i] = x;
                #[allow(clippy::cast_possible_truncation)]
                let c: Vec<i32> = v.crop.iter().map(|c| (c * 1000.0).round() as i32).collect();
                vec![json!({ "cmd": "pictureCrop", "crop": c })]
            }
            "tlineW" => {
                v.text_line_w = x;
                vec![json!({ "cmd": "shapeTextOutline", "weight": x })]
            }
            _ => Vec::new(),
        }
    }

    fn shadow_preset(&self) -> String {
        let s = self
            .vals
            .shadow
            .unwrap_or([60.0, 100.0, 4.0, 45.0, 3.0, 0.0]);
        format!(
            "custom:inner={};color={:06X};transparency={};size={};blur={};angle={};distance={}",
            u8::from(self.vals.shadow_inner),
            self.vals.shadow_color,
            fmt(s[0]),
            fmt(s[1]),
            fmt(s[2]),
            fmt(s[3]),
            fmt(s[4])
        )
    }

    fn reflection_preset(&self) -> String {
        let r = self.vals.reflection.unwrap_or([48.0, 55.0, 0.5, 0.0]);
        format!(
            "custom:transparency={};size={};blur={};distance={}",
            fmt(r[0]),
            fmt(r[1]),
            fmt(r[2]),
            fmt(r[3])
        )
    }

    fn glow_preset(&self) -> String {
        let g = self.vals.glow.unwrap_or([8.0, 60.0]);
        format!(
            "custom:size={};color={:06X};transparency={}",
            fmt(g[0]),
            self.vals.glow_color,
            fmt(g[1])
        )
    }

    fn gradient_cmd(&self) -> Value {
        let v = &self.vals;
        let stops: Vec<Value> = v
            .stops
            .iter()
            .map(|(p, c, t)| json!({ "pos": p, "color": format!("{c:06X}"), "alpha": 100.0 - t }))
            .collect();
        let kind = ["linear", "circle", "rect", "shape"][v.grad_kind.min(3)];
        json!({
            "cmd": "shapeGradient",
            "focus": v.grad_focus,
            "kind": kind,
            "angle": v.grad_angle,
            "stops": stops,
        })
    }

    fn fx_cmd(&self) -> Value {
        let f = &self.vals.fx;
        #[allow(clippy::cast_possible_truncation)]
        let k = |x: f64| (x * 1000.0).round() as i32;
        let mut fx = self.vals.raw_fx.clone();
        fx.sharpen = k(f[0]);
        fx.bright = k(f[1]);
        fx.contrast = k(f[2]);
        fx.saturation = ((f[3] - 100.0).abs() >= 0.01).then(|| k(f[3]));
        fx.alpha = (f[4] > 0.0).then(|| k(100.0 - f[4]));
        json!({ "cmd": "pictureFx", "fx": fx })
    }

    /// A colour chosen for control `id` (`RRGGBB`).
    pub(crate) fn set_color(&mut self, id: &str, rgb: u32) -> Vec<Value> {
        let hex = format!("{rgb:06X}");
        let shape = self.target == Target::Shape;
        let v = &mut self.vals;
        match id {
            "fillColor" => {
                v.fill_color = rgb;
                vec![json!({ "cmd": "pictureShapeFill", "color": hex })]
            }
            "lineColor" => {
                v.line_color = rgb;
                vec![if shape {
                    json!({ "cmd": "pictureShapeOutline", "color": hex })
                } else {
                    json!({ "cmd": "pictureBorder", "color": hex })
                }]
            }
            "stopColor" => {
                if let Some(s) = v.stops.get_mut(v.stop) {
                    s.1 = rgb;
                }
                self.bright = (usize::MAX, 0, 0.0);
                vec![self.gradient_cmd()]
            }
            "patFg" | "patBg" => {
                let mut pat = self.pattern.unwrap_or((0, 0x44_72C4, 0xFF_FFFF));
                if id == "patFg" {
                    pat.1 = rgb;
                } else {
                    pat.2 = rgb;
                }
                self.pattern = Some(pat);
                vec![self.pattern_cmd()]
            }
            "shadowColor" => {
                v.shadow_color = rgb;
                let kind = if shape {
                    "shapeEffect"
                } else {
                    "pictureEffect"
                };
                vec![json!({ "cmd": kind, "kind": "shadow", "preset": self.shadow_preset() })]
            }
            "glowColor" => {
                v.glow_color = rgb;
                let kind = if shape {
                    "shapeEffect"
                } else {
                    "pictureEffect"
                };
                vec![json!({ "cmd": kind, "kind": "glow", "preset": self.glow_preset() })]
            }
            "tfillColor" => {
                v.text_fill = Some(rgb);
                vec![json!({ "cmd": "shapeTextFill", "color": hex })]
            }
            "tlineColor" => {
                v.text_line = Some(rgb);
                vec![json!({ "cmd": "shapeTextOutline", "color": hex })]
            }
            _ => Vec::new(),
        }
    }

    /// Entries of the drop-down of control `id`: (choice id, label key, English, checked).
    pub(crate) fn menu(&self, id: &str) -> Vec<(String, String, String, bool)> {
        let v = &self.vals;
        let e = |c: String, k: &str, en: &str, on: bool| (c, k.to_owned(), en.to_owned(), on);
        let none = |kind: &str| {
            e(
                format!("fp21.p:{kind}=none"),
                "doc21.fpNone",
                "No effect",
                false,
            )
        };
        match id {
            "gradKind" => GRAD_KINDS
                .iter()
                .enumerate()
                .map(|(i, (k, en))| e(format!("fp21.gk:{i}"), k, en, v.grad_kind == i))
                .collect(),
            "dash" => DASHES
                .iter()
                .enumerate()
                .map(|(i, d)| e(format!("fp21.dash:{i}"), "", &words(d), v.dash == i))
                .collect(),
            "anchor" => ANCHORS
                .iter()
                .enumerate()
                .map(|(i, (_, k, en))| e(format!("fp21.anchor:{i}"), k, en, v.anchor == i))
                .collect(),
            "shadowPresets" | "tshadow" => std::iter::once(none("shadow"))
                .chain(
                    bo_engine::doc_picstyle::SHADOWS
                        .iter()
                        .map(|s| e(format!("fp21.p:shadow={}", s.0), "", &words(s.0), false)),
                )
                .collect(),
            "reflPresets" | "treflection" => std::iter::once(none("reflection"))
                .chain(
                    bo_engine::doc_picstyle::REFLECTIONS
                        .iter()
                        .map(|s| e(format!("fp21.p:reflection={}", s.0), "", &words(s.0), false)),
                )
                .collect(),
            "glowPresets" | "tglow" => {
                let mut out = vec![none("glow")];
                for size in bo_engine::doc_picstyle::GLOW_SIZES {
                    for c in bo_engine::doc_picstyle::GLOW_COLORS {
                        out.push(e(
                            format!("fp21.p:glow={}:{c:06X}", fmt(size)),
                            "",
                            &format!("{} pt, #{c:06X}", fmt(size)),
                            false,
                        ));
                    }
                }
                out
            }
            "softPresets" => std::iter::once(none("softEdge"))
                .chain(bo_engine::doc_picstyle::SOFT_EDGES.iter().map(|s| {
                    e(
                        format!("fp21.p:softEdge={}", fmt(*s)),
                        "",
                        &format!("{} pt", fmt(*s)),
                        false,
                    )
                }))
                .collect(),
            "bevel" => std::iter::once(none("bevel"))
                .chain(bo_engine::doc_picstyle::BEVELS.iter().map(|b| {
                    e(
                        format!("fp21.p:bevel={b}"),
                        "",
                        &words(b),
                        v.bevel.as_ref().is_some_and(|x| x.0 == *b),
                    )
                }))
                .collect(),
            "rotation" => std::iter::once(none("rotation"))
                .chain(bo_engine::doc_picstyle::ROTATIONS.iter().map(|r| {
                    e(
                        format!("fp21.p:rotation={r}"),
                        "",
                        &words(r),
                        v.camera.as_deref() == Some(*r),
                    )
                }))
                .collect(),
            "texture" => (0..25)
                .map(|i| {
                    e(
                        format!("r20.tex:{i}"),
                        &format!("doc20.tex{i}"),
                        &format!("Texture {}", i + 1),
                        false,
                    )
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// A drop-down entry (`fp21.*`) was chosen: the commands it runs.
    pub(crate) fn choose(&mut self, choice: &str) -> Vec<Value> {
        let shape = self.target == Target::Shape;
        let fx_cmd = if self.text_tab {
            "shapeTextEffect"
        } else if shape {
            "shapeEffect"
        } else {
            "pictureEffect"
        };
        if let Some(rest) = choice.strip_prefix("fp21.gv:") {
            let (var, i) = rest.split_once(':').unwrap_or((rest, "0"));
            self.vals.fill = 2;
            self.pattern = None;
            return vec![json!({
                "cmd": "shapeGradient", "variation": var, "index": i.parse::<u64>().unwrap_or(0)
            })];
        }
        if let Some(a) = choice.strip_prefix("fp21.ga:") {
            self.vals.grad_angle = a.parse().unwrap_or(90.0);
            return vec![self.gradient_cmd()];
        }
        if let Some(f) = choice.strip_prefix("fp21.gf:") {
            let n: Vec<f64> = f.split(',').filter_map(|x| x.parse().ok()).collect();
            if let Ok(f) = <[f64; 4]>::try_from(n) {
                self.vals.grad_focus = f;
            }
            return vec![self.gradient_cmd()];
        }
        if let Some(i) = choice.strip_prefix("fp21.gk:") {
            self.vals.grad_kind = i.parse().unwrap_or(0).min(3);
            return vec![self.gradient_cmd()];
        }
        if let Some(i) = choice.strip_prefix("fp21.dash:") {
            self.vals.dash = i.parse().unwrap_or(0).min(DASHES.len() - 1);
            let d = DASHES[self.vals.dash];
            return vec![if shape {
                json!({ "cmd": "pictureShapeOutline", "dash": d })
            } else {
                json!({ "cmd": "pictureBorder", "dash": d })
            }];
        }
        if let Some(i) = choice.strip_prefix("fp21.anchor:") {
            self.vals.anchor = i.parse().unwrap_or(0).min(2);
            return vec![
                json!({ "cmd": "shapeTextAnchor", "anchor": ANCHORS[self.vals.anchor].0 }),
            ];
        }
        if let Some(rest) = choice.strip_prefix("fp21.p:") {
            let (kind, preset) = rest.split_once('=').unwrap_or((rest, "none"));
            if self.text_tab {
                return vec![
                    json!({ "cmd": fx_cmd, "kind": if preset == "none" { "none" } else { kind }, "preset": preset }),
                ];
            }
            return vec![json!({ "cmd": fx_cmd, "kind": kind, "preset": preset })];
        }
        Vec::new()
    }

    // -------------------------------------------------------------- geometry

    fn px(&self, v: f32) -> f32 {
        (v * self.scale).round()
    }

    /// Top of the scrolled content (window px).
    fn content_top(&self) -> f32 {
        self.rect[1]
            + self.px(if self.target == Target::Shape {
                150.0
            } else {
                108.0
            })
    }

    /// Right-aligned box of a row (window px).
    fn box_rect(&self, row: &Row) -> [f32; 4] {
        let y = self.content_top() + self.px(row.y) - self.scroll;
        let (w, h) = match row.kind {
            RowKind::Color(..) => (self.px(36.0), self.px(23.0)),
            RowKind::Choice(..) => (self.px(98.0), self.px(23.0)),
            RowKind::Button(..) => (self.px(80.0), self.px(25.0)),
            _ => (self.px(86.0), self.px(23.0)),
        };
        let x = self.rect[0] + self.rect[2] - self.px(32.0) - w;
        [x, (y + (self.px(row.h) - h) / 2.0).round(), w, h]
    }

    /// Icon boxes of the pages (window px).
    fn page_rects(&self) -> Vec<(Page, [f32; 4])> {
        let y = self.rect[1]
            + self.px(if self.target == Target::Shape {
                90.0
            } else {
                52.0
            });
        self.pages()
            .into_iter()
            .enumerate()
            .map(|(i, p)| {
                (
                    p,
                    [
                        self.rect[0] + self.px(13.0 + 45.0 * i as f32),
                        y,
                        self.px(37.0),
                        self.px(37.0),
                    ],
                )
            })
            .collect()
    }

    fn close_rect(&self) -> [f32; 4] {
        [
            self.rect[0] + self.rect[2] - self.px(30.0),
            self.rect[1] + self.px(8.0),
            self.px(24.0),
            self.px(24.0),
        ]
    }

    fn tab_rects(&self) -> [[f32; 4]; 2] {
        let y = self.rect[1] + self.px(48.0);
        [
            [
                self.rect[0] + self.px(13.0),
                y,
                self.px(108.0),
                self.px(30.0),
            ],
            [
                self.rect[0] + self.px(124.0),
                y,
                self.px(100.0),
                self.px(30.0),
            ],
        ]
    }

    // -------------------------------------------------------------- input

    /// How far the rows scroll.
    fn max_scroll(&self) -> f32 {
        let total: f32 = self.rows().iter().map(|r| r.h).sum::<f32>() * self.scale;
        let room = self.rect[1] + self.rect[3] - self.content_top();
        (total - room).max(0.0)
    }

    /// Track and thumb of the scroll bar (window px) when the rows overflow.
    fn scroll_bar(&self) -> Option<([f32; 4], [f32; 4])> {
        let max = self.max_scroll();
        if max <= 0.0 {
            return None;
        }
        let top = self.content_top();
        let room = self.rect[1] + self.rect[3] - top;
        let w = self.px(10.0);
        let track = [self.rect[0] + self.rect[2] - w - self.px(4.0), top, w, room];
        let th = (room * room / (room + max)).max(self.px(24.0));
        let ty = top + (room - th) * (self.scroll / max);
        Some((track, [track[0], ty, w, th]))
    }

    /// The slider left of spin box `id` (window px), for the settings the reference gives one.
    fn slider_rect(&self, id: &str, b: [f32; 4]) -> Option<[f32; 4]> {
        let with = matches!(
            id,
            "fillTransp"
                | "lineTransp"
                | "stopTransp"
                | "stopBright"
                | "shTransp"
                | "shSize"
                | "shBlur"
                | "shAngle"
                | "shDist"
                | "rfTransp"
                | "rfSize"
                | "rfBlur"
                | "rfDist"
                | "glSize"
                | "glTransp"
                | "softSize"
                | "fxSharp"
                | "fxBright"
                | "fxContrast"
                | "fxSat"
                | "fxTransp"
        );
        with.then(|| {
            let w = self.px(28.0);
            [
                b[0] - w - self.px(10.0),
                b[1] + self.px(4.0),
                w,
                b[3] - self.px(8.0),
            ]
        })
    }

    /// The title's ▾ (Move / Size / Close menu of a task pane).
    fn menu_rect(&self) -> [f32; 4] {
        let c = self.close_rect();
        [c[0] - self.px(30.0), c[1], c[2], c[3]]
    }

    /// The pattern grid of a Patterns row (window px).
    fn pattern_rect(&self, row: &Row) -> [f32; 4] {
        let y = self.content_top() + self.px(row.y) - self.scroll + self.px(4.0);
        let x0 = self.rect[0] + self.px(36.0);
        let w = (self.px(22.0) * 8.0).min(self.rect[2] - self.px(36.0 + 32.0));
        [x0, y, w, w / 8.0 * 6.0]
    }

    pub(crate) fn wheel(&mut self, dy: f32) -> bool {
        let max = self.max_scroll();
        let s = (self.scroll - dy * self.px(40.0)).clamp(0.0, max);
        std::mem::replace(&mut self.scroll, s) != s
    }

    pub(crate) fn hover(&mut self, x: f32, y: f32) -> bool {
        let h = self.contains(x, y).then_some((x, y));
        std::mem::replace(&mut self.hover, h) != h
    }

    /// A press at (x, y) window px.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn press(&mut self, x: f32, y: f32) -> PaneAct {
        let inr = |r: [f32; 4]| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
        if inr(self.close_rect()) {
            return PaneAct::Close;
        }
        let m = self.menu_rect();
        if inr(m) {
            return PaneAct::Menu("paneMenu", m);
        }
        // The scroll bar: a press pages towards it.
        if let Some((track, thumb)) = self.scroll_bar() {
            if inr(track) {
                let page = track[3] * 0.8;
                let dy = if y < thumb[1] {
                    -page
                } else if y > thumb[1] + thumb[3] {
                    page
                } else {
                    0.0
                };
                let max = self.max_scroll();
                self.scroll = (self.scroll + dy).clamp(0.0, max);
                return PaneAct::Redraw;
            }
        }
        if self.target == Target::Shape {
            let t = self.tab_rects();
            if inr(t[0]) || inr(t[1]) {
                let text = inr(t[1]);
                if text != self.text_tab {
                    self.text_tab = text;
                    self.page = Page::FillLine;
                    self.scroll = 0.0;
                }
                return PaneAct::Redraw;
            }
        }
        if let Some((p, _)) = self.page_rects().into_iter().find(|(_, r)| inr(*r)) {
            self.page = p;
            self.scroll = 0.0;
            return PaneAct::Redraw;
        }
        if y < self.content_top() {
            return PaneAct::None;
        }
        let rows = self.rows();
        let top = self.content_top() - self.scroll;
        let Some(row) = rows
            .iter()
            .find(|r| y >= top + self.px(r.y) && y < top + self.px(r.y + r.h))
            .cloned()
        else {
            return PaneAct::None;
        };
        let b = self.box_rect(&row);
        match row.kind {
            RowKind::Section(id, ..) => {
                if !self.open.remove(id) {
                    self.open.insert(id);
                }
                PaneAct::Redraw
            }
            RowKind::Radio(group, val, ..) => self.radio(group, val),
            RowKind::Color(id, ..) | RowKind::Choice(id, ..) if inr(b) => PaneAct::Menu(id, b),
            RowKind::Button("picInsert", ..) if inr(b) => PaneAct::PickPicture,
            RowKind::Button("fxReset", ..) if inr(b) => {
                self.vals.fx = [0.0, 0.0, 0.0, 100.0, 0.0];
                PaneAct::Run(vec![self.fx_cmd()])
            }
            RowKind::Spin("gradAngle", ..) if self.vals.grad_kind != 0 => PaneAct::None,
            RowKind::Patterns => {
                let g = self.pattern_rect(&row);
                let cell = g[2] / 8.0;
                if x < g[0] || y < g[1] {
                    return PaneAct::None;
                }
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let (cx, cy) = (((x - g[0]) / cell) as usize, ((y - g[1]) / cell) as usize);
                let i = cy * 8 + cx;
                if cx >= 8 || i >= crate::slide_bgpane::PATTERNS.len() {
                    return PaneAct::None;
                }
                let mut pat = self.pattern.unwrap_or((0, self.vals.fill_color, 0xFF_FFFF));
                pat.0 = i;
                self.pattern = Some(pat);
                PaneAct::Run(vec![self.pattern_cmd()])
            }
            RowKind::Spin(id, ..) if self.slider_rect(id, b).is_some_and(inr) => {
                let r = self.slider_rect(id, b).unwrap_or(b);
                let (_, _, lo, hi, _) = self.spin(id);
                let f = f64::from(((x - r[0]) / r[2]).clamp(0.0, 1.0));
                PaneAct::Run(self.set_spin(id, (lo + (hi - lo) * f).round()))
            }
            RowKind::Spin(id, ..) => {
                let (val, step, ..) = self.spin(id);
                match spin_hit(b, self.scale, x, y) {
                    Some(SpinHit::Up) => PaneAct::Run(self.set_spin(id, val + step)),
                    Some(SpinHit::Down) => PaneAct::Run(self.set_spin(id, val - step)),
                    _ => PaneAct::None,
                }
            }
            RowKind::Stops => self.press_stops(x, y, &row),
            _ => PaneAct::None,
        }
    }

    /// The gradient stop bar: pick a stop; the add / remove buttons at its right.
    fn press_stops(&mut self, x: f32, y: f32, row: &Row) -> PaneAct {
        let (bar, add, remove) = self.stop_rects(row);
        let inr = |r: [f32; 4]| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3];
        if inr(add) {
            let (a, b) = (
                self.vals.stops.get(self.vals.stop).copied(),
                self.vals.stops.get(self.vals.stop + 1).copied(),
            );
            let new = match (a, b) {
                (Some(a), Some(b)) => (f64::midpoint(a.0, b.0), a.1, a.2),
                (Some(a), None) => (f64::midpoint(a.0, 100.0), a.1, a.2),
                _ => (50.0, 0x80_8080, 0.0),
            };
            let at = (self.vals.stop + 1).min(self.vals.stops.len());
            self.vals.stops.insert(at, new);
            self.vals.stop = at;
            return PaneAct::Run(vec![self.gradient_cmd()]);
        }
        if inr(remove) {
            if self.vals.stops.len() > 2 {
                self.vals.stops.remove(self.vals.stop);
                self.vals.stop = self.vals.stop.min(self.vals.stops.len() - 1);
                return PaneAct::Run(vec![self.gradient_cmd()]);
            }
            return PaneAct::None;
        }
        if inr([
            bar[0] - self.px(6.0),
            bar[1],
            bar[2] + self.px(12.0),
            bar[3] + self.px(14.0),
        ]) {
            let pos = f64::from(((x - bar[0]) / bar[2]).clamp(0.0, 1.0) * 100.0);
            if let Some(i) = self
                .vals
                .stops
                .iter()
                .enumerate()
                .min_by(|a, b| (a.1 .0 - pos).abs().total_cmp(&(b.1 .0 - pos).abs()))
                .map(|(i, _)| i)
            {
                self.vals.stop = i;
            }
            return PaneAct::Redraw;
        }
        PaneAct::None
    }

    fn stop_rects(&self, row: &Row) -> ([f32; 4], [f32; 4], [f32; 4]) {
        let y = self.content_top() + self.px(row.y) - self.scroll + self.px(22.0);
        let x0 = self.rect[0] + self.px(36.0);
        let bw = self.rect[2] - self.px(36.0 + 32.0 + 62.0);
        let bar = [x0, y, bw, self.px(14.0)];
        let add = [
            x0 + bw + self.px(10.0),
            y - self.px(3.0),
            self.px(24.0),
            self.px(24.0),
        ];
        let remove = [add[0] + self.px(28.0), add[1], add[2], add[3]];
        (bar, add, remove)
    }

    /// Fill › `kind` (2 gradient, 3 picture or texture) chosen from outside the pane (Shape
    /// Fill › More Gradients… / More Textures…).
    pub(crate) fn choose_fill(&mut self, kind: u8) -> PaneAct {
        self.open.insert("fill");
        self.radio("fill", kind)
    }

    /// An option button of `group` set to `val`.
    fn radio(&mut self, group: &str, val: u8) -> PaneAct {
        let shape = self.target == Target::Shape;
        if group == "fill" {
            if val == 4 {
                let c = self.vals.fill_color;
                self.pattern = Some(self.pattern.unwrap_or((0, c, 0xFF_FFFF)));
                self.vals.fill = 3;
                return PaneAct::Run(vec![self.pattern_cmd()]);
            }
            self.pattern = None;
        }
        let v = &mut self.vals;
        let cmds = match (group, val) {
            ("fill", 0) => {
                v.fill = 0;
                vec![json!({ "cmd": "pictureShapeFill", "color": "none" })]
            }
            ("fill", 1) => {
                v.fill = 1;
                vec![
                    json!({ "cmd": "shapeGradient", "variation": "none" }),
                    json!({ "cmd": "pictureShapeFill", "color": format!("{:06X}", v.fill_color) }),
                ]
            }
            ("fill", 2) => {
                v.fill = 2;
                if v.stops.len() < 2 {
                    v.stops = vec![(0.0, v.fill_color, 0.0), (100.0, 0xFF_FFFF, 0.0)];
                }
                vec![self.gradient_cmd()]
            }
            ("fill", 3) => {
                v.fill = 3;
                return PaneAct::Redraw;
            }
            ("line", 0) => {
                v.line = 0;
                vec![if shape {
                    json!({ "cmd": "pictureShapeOutline", "color": "none" })
                } else {
                    json!({ "cmd": "pictureBorder", "color": "none" })
                }]
            }
            ("line", _) => {
                v.line = 1;
                let hex = format!("{:06X}", v.line_color);
                vec![if shape {
                    json!({ "cmd": "pictureShapeOutline", "color": hex, "weight": v.line_width.max(0.25) })
                } else {
                    json!({ "cmd": "pictureBorder", "color": hex, "weight": v.line_width.max(0.25) })
                }]
            }
            ("tfill", 0) => {
                v.text_fill = None;
                vec![json!({ "cmd": "shapeTextFill", "color": "none" })]
            }
            ("tfill", _) => {
                let c = v.text_fill.unwrap_or(0);
                v.text_fill = Some(c);
                vec![json!({ "cmd": "shapeTextFill", "color": format!("{c:06X}") })]
            }
            ("tline", 0) => {
                v.text_line = None;
                vec![json!({ "cmd": "shapeTextOutline", "color": "none" })]
            }
            ("tline", _) => {
                let c = v.text_line.unwrap_or(0x44_72C4);
                v.text_line = Some(c);
                vec![
                    json!({ "cmd": "shapeTextOutline", "color": format!("{c:06X}"), "weight": v.text_line_w }),
                ]
            }
            _ => return PaneAct::None,
        };
        PaneAct::Run(cmds)
    }

    // -------------------------------------------------------------- painting

    /// Paint the pane; `t` translates (key, English).
    #[allow(clippy::too_many_lines)]
    pub(crate) fn paint(&self, p: &mut dyn Painter, t: &dyn Fn(&str, &str) -> String) {
        let [x0, y0, w, h] = self.rect;
        let bg = Rgba(0xF3, 0xF3, 0xF3, 255);
        let ink = Rgba(0x26, 0x26, 0x26, 255);
        let grey = Rgba(0x60, 0x60, 0x60, 255);
        let accent = Rgba(0x2B, 0x57, 0x9A, 255);
        let edge = Rgba(0xC8, 0xC8, 0xC8, 255);
        p.fill_rect(x0, y0, w, h, bg);
        p.fill_rect(x0, y0, 1.0, h, edge);
        let size = self.px(12.0);
        let st = TextStyle { size, bold: false };
        let bold = TextStyle { size, bold: true };
        let line = size * 1.33;
        let title = match self.target {
            _ if self.text_tab => t("doc21.fpTitleText", "Format Shape"),
            Target::Shape => t("doc21.fpTitleShape", "Format Shape"),
            Target::Picture => t("doc21.fpTitlePicture", "Format Picture"),
        };
        p.text(
            &title,
            x0 + self.px(12.0),
            y0 + self.px(8.0),
            TextStyle {
                size: self.px(20.0),
                bold: false,
            },
            ink,
        );
        // Close (×).
        let c = self.close_rect();
        let k = self.scale;
        p.path(
            &[
                PathSeg::M(c[0] + 7.0 * k, c[1] + 7.0 * k),
                PathSeg::L(c[0] + c[2] - 7.0 * k, c[1] + c[3] - 7.0 * k),
                PathSeg::M(c[0] + c[2] - 7.0 * k, c[1] + 7.0 * k),
                PathSeg::L(c[0] + 7.0 * k, c[1] + c[3] - 7.0 * k),
            ],
            None,
            Some((grey, 1.2 * k)),
        );
        // Task pane options ▾.
        let m = self.menu_rect();
        let (mx, my) = (m[0] + m[2] / 2.0, m[1] + m[3] / 2.0);
        p.path(
            &[
                PathSeg::M(mx - 5.0 * k, my - 2.5 * k),
                PathSeg::L(mx, my + 2.5 * k),
                PathSeg::L(mx + 5.0 * k, my - 2.5 * k),
            ],
            None,
            Some((grey, 1.2 * k)),
        );
        if self.target == Target::Shape {
            let tr = self.tab_rects();
            for (i, (key, en)) in [
                ("doc21.fpShapeOptions", "Shape Options"),
                ("doc21.fpTextOptions", "Text Options"),
            ]
            .iter()
            .enumerate()
            {
                let on = self.text_tab == (i == 1);
                p.text(
                    &t(key, en),
                    tr[i][0] + self.px(4.0),
                    tr[i][1] + (tr[i][3] - line) / 2.0,
                    if on { bold } else { st },
                    ink,
                );
                if on {
                    p.fill_rect(
                        tr[i][0],
                        tr[i][1] + tr[i][3],
                        tr[i][2],
                        self.px(3.0),
                        accent,
                    );
                }
            }
        }
        for (pg, r) in self.page_rects() {
            if pg == self.page {
                p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0x40, 0x40, 0x40, 255));
                p.fill_rect(
                    r[0] + 1.0,
                    r[1] + 1.0,
                    r[2] - 2.0,
                    r[3] - 2.0,
                    Rgba(0xD2, 0xD2, 0xD2, 255),
                );
            }
            page_icon(p, pg, r, k, accent);
        }
        let sep_y = self.content_top() - self.px(10.0);
        p.fill_rect(x0 + self.px(10.0), sep_y, w - self.px(20.0), 1.0, edge);
        // Rows.
        let clip_top = self.content_top();
        p.push_clip(x0, clip_top, w, y0 + h - clip_top);
        for row in self.rows() {
            let ry = self.content_top() + self.px(row.y) - self.scroll;
            let rh = self.px(row.h);
            if ry + rh < clip_top || ry > y0 + h {
                continue;
            }
            let ty = (ry + (rh - line) / 2.0).round();
            let lx = x0 + self.px(36.0);
            let b = self.box_rect(&row);
            match &row.kind {
                RowKind::Section(id, key, en) => {
                    let open = self.open.contains(id);
                    let cx = x0 + self.px(18.0);
                    let cy = ry + rh / 2.0;
                    let s = 4.0 * k;
                    let segs = if open {
                        [
                            PathSeg::M(cx - s, cy - s / 2.0),
                            PathSeg::L(cx, cy + s / 2.0),
                            PathSeg::L(cx + s, cy - s / 2.0),
                        ]
                    } else {
                        [
                            PathSeg::M(cx - s / 2.0, cy - s),
                            PathSeg::L(cx + s / 2.0, cy),
                            PathSeg::L(cx - s / 2.0, cy + s),
                        ]
                    };
                    p.path(&segs, None, Some((ink, 1.3 * k)));
                    p.text(&t(key, en), x0 + self.px(30.0), ty, bold, ink);
                }
                RowKind::Radio(group, val, key, en) => {
                    let on = match *group {
                        "fill" => self.fill_kind() == *val,
                        "line" => self.vals.line == *val,
                        "tfill" => self.vals.text_fill.is_some() == (*val == 1),
                        "tline" => self.vals.text_line.is_some() == (*val == 1),
                        _ => false,
                    };
                    let d = self.px(13.0);
                    let (cx, cy) = (lx + d / 2.0, ry + rh / 2.0);
                    circle(
                        p,
                        cx,
                        cy,
                        d / 2.0,
                        Some(Rgba(0xFF, 0xFF, 0xFF, 255)),
                        Some((grey, k)),
                    );
                    if on {
                        circle(p, cx, cy, d / 4.5, Some(accent), None);
                    }
                    p.text(&t(key, en), lx + d + self.px(8.0), ty, st, ink);
                }
                RowKind::Color(id, key, en) => {
                    p.text(&t(key, en), lx, ty, st, ink);
                    let rgb = self.color_of(id);
                    p.fill_rect(b[0], b[1], b[2], b[3], Rgba(0x8A, 0x8A, 0x8A, 255));
                    p.fill_rect(
                        b[0] + 1.0,
                        b[1] + 1.0,
                        b[2] - 2.0,
                        b[3] - 2.0,
                        Rgba(0xFF, 0xFF, 0xFF, 255),
                    );
                    let sw = b[2] - self.px(14.0);
                    p.fill_rect(
                        b[0] + self.px(3.0),
                        b[1] + self.px(4.0),
                        sw - self.px(3.0),
                        b[3] - self.px(8.0),
                        rgba(rgb),
                    );
                    // Drop-down mark.
                    let mx = b[0] + b[2] - self.px(7.0);
                    let my = b[1] + b[3] / 2.0;
                    p.path(
                        &[
                            PathSeg::M(mx - 3.0 * k, my - 1.5 * k),
                            PathSeg::L(mx + 3.0 * k, my - 1.5 * k),
                            PathSeg::L(mx, my + 2.0 * k),
                            PathSeg::Z,
                        ],
                        Some(ink),
                        None,
                    );
                }
                RowKind::Choice(id, key, en) => {
                    p.text(&t(key, en), lx, ty, st, ink);
                    let text = self.choice_text(id, t);
                    let swatch = matches!(*id, "gradPreset" | "gradDir");
                    paint_combo(
                        p,
                        b,
                        self.scale,
                        &BoxLook {
                            text: &text,
                            style: st,
                            ink,
                            enabled: true,
                            caret: None,
                            hover: None,
                        },
                    );
                    if swatch {
                        let sw = [
                            b[0] + self.px(4.0),
                            b[1] + self.px(4.0),
                            self.px(40.0),
                            b[3] - self.px(8.0),
                        ];
                        let n = 16;
                        for q in 0..n {
                            let c = self.color_at(f64::from(q) / f64::from(n - 1) * 100.0);
                            p.fill_rect(
                                sw[0] + sw[2] * q as f32 / n as f32,
                                sw[1],
                                sw[2] / n as f32 + 1.0,
                                sw[3],
                                rgba(c),
                            );
                        }
                    }
                }
                RowKind::Spin(id, key, en) => {
                    p.text(&t(key, en), lx, ty, st, ink);
                    if let Some(r) = self.slider_rect(id, b) {
                        // The slider beside the box, as the reference's effect settings.
                        let (v, _, lo, hi, _) = self.spin(id);
                        let cy = r[1] + r[3] / 2.0;
                        p.fill_rect(r[0], cy.round(), r[2], 1.0, Rgba(0x80, 0x80, 0x80, 255));
                        #[allow(clippy::cast_possible_truncation)]
                        let f = (((v - lo) / (hi - lo).max(1e-9)).clamp(0.0, 1.0)) as f32;
                        let tx = (r[0] + r[2] * f - 2.0 * k).round();
                        p.fill_rect(
                            tx,
                            r[1],
                            (4.0 * k).round(),
                            r[3],
                            Rgba(0x40, 0x40, 0x40, 255),
                        );
                    }
                    let (val, _, _, _, unit) = self.spin(id);
                    let unset = match &id[..2] {
                        "sh" => self.vals.shadow.is_none(),
                        "rf" => self.vals.reflection.is_none(),
                        "gl" => self.vals.glow.is_none(),
                        "be" => self.vals.bevel.is_none(),
                        _ => false,
                    };
                    let text = if unset {
                        String::new()
                    } else {
                        format!("{}{unit}", fmt(val))
                    };
                    let hover = self
                        .hover
                        .and_then(|(hx, hy)| spin_hit(b, self.scale, hx, hy));
                    paint_spin(
                        p,
                        b,
                        self.scale,
                        &BoxLook {
                            text: &text,
                            style: st,
                            ink,
                            enabled: !(*id == "gradAngle" && self.vals.grad_kind != 0),
                            caret: None,
                            hover,
                        },
                    );
                }
                RowKind::Patterns => {
                    let g = self.pattern_rect(&row);
                    let cell = g[2] / 8.0;
                    let (sel, fg, bg) = self.pattern.unwrap_or((usize::MAX, 0x44_72C4, 0xFF_FFFF));
                    for (i, name) in crate::slide_bgpane::PATTERNS.iter().enumerate() {
                        let (cx, cy) = (g[0] + cell * (i % 8) as f32, g[1] + cell * (i / 8) as f32);
                        let r = [cx + 1.0, cy + 1.0, cell - 2.0, cell - 2.0];
                        p.fill_rect(r[0], r[1], r[2], r[3], rgba(bg));
                        let bits = crate::slide_bgpane::pattern_bits(name);
                        let d = r[2] / 8.0;
                        for (ry, row_bits) in bits.iter().enumerate() {
                            for bx in 0..8 {
                                if row_bits & (0x80 >> bx) != 0 {
                                    p.fill_rect(
                                        r[0] + d * bx as f32,
                                        r[1] + d * ry as f32,
                                        d,
                                        d,
                                        rgba(fg),
                                    );
                                }
                            }
                        }
                        let edge_c = if i == sel {
                            Rgba(0xD2, 0x6E, 0x00, 255)
                        } else {
                            Rgba(0xA0, 0xA0, 0xA0, 255)
                        };
                        let t = if i == sel { 2.0 } else { 1.0 };
                        p.fill_rect(cx, cy, cell, t, edge_c);
                        p.fill_rect(cx, cy + cell - t, cell, t, edge_c);
                        p.fill_rect(cx, cy, t, cell, edge_c);
                        p.fill_rect(cx + cell - t, cy, t, cell, edge_c);
                    }
                }
                RowKind::Button(_, key, en) => {
                    p.fill_rect(b[0], b[1], b[2], b[3], Rgba(0xAD, 0xAD, 0xAD, 255));
                    p.fill_rect(
                        b[0] + 1.0,
                        b[1] + 1.0,
                        b[2] - 2.0,
                        b[3] - 2.0,
                        Rgba(0xE1, 0xE1, 0xE1, 255),
                    );
                    p.text(
                        &t(key, en),
                        b[0] + self.px(8.0),
                        (b[1] + (b[3] - line) / 2.0).round(),
                        st,
                        ink,
                    );
                }
                RowKind::Stops => {
                    p.text(
                        &t("doc21.fpStops", "Gradient stops"),
                        lx,
                        ry + self.px(2.0),
                        st,
                        ink,
                    );
                    let (bar, add, remove) = self.stop_rects(&row);
                    // The gradient itself, then the stop markers.
                    let n = 48;
                    for i in 0..n {
                        let f = i as f64 / f64::from(n - 1) * 100.0;
                        let c = self.color_at(f);
                        let sx = bar[0] + bar[2] * i as f32 / n as f32;
                        p.fill_rect(sx, bar[1], bar[2] / n as f32 + 1.0, bar[3], rgba(c));
                    }
                    for (i, s) in self.vals.stops.iter().enumerate() {
                        let sx = bar[0] + bar[2] * (s.0 / 100.0) as f32;
                        let sel = i == self.vals.stop;
                        // A pentagon marker pointing down at the bar's bottom edge, the
                        // chosen one outlined (BlueOffice's own drawing).
                        let top = bar[1] - 2.0 * k;
                        p.path(
                            &stop_marker(sx, top, k),
                            Some(rgba(s.1)),
                            Some(if sel {
                                (Rgba(0xD2, 0x6E, 0x00, 255), 2.0 * k)
                            } else {
                                (grey, k)
                            }),
                        );
                    }
                    for (r, plus) in [(add, true), (remove, false)] {
                        p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xAD, 0xAD, 0xAD, 255));
                        p.fill_rect(
                            r[0] + 1.0,
                            r[1] + 1.0,
                            r[2] - 2.0,
                            r[3] - 2.0,
                            Rgba(0xFF, 0xFF, 0xFF, 255),
                        );
                        // A small stop marker with a + (add) or a × (remove).
                        p.path(
                            &stop_marker(r[0] + 9.0 * k, r[1] + 4.0 * k, k),
                            Some(Rgba(0xFF, 0xFF, 0xFF, 255)),
                            Some((Rgba(0x60, 0x60, 0x60, 255), k)),
                        );
                        let (cx, cy) = (r[0] + 17.0 * k, r[1] + 8.0 * k);
                        if plus {
                            let g = Rgba(0x10, 0x7C, 0x10, 255);
                            p.fill_rect(cx - 3.5 * k, cy - 0.75 * k, 7.0 * k, 1.5 * k, g);
                            p.fill_rect(cx - 0.75 * k, cy - 3.5 * k, 1.5 * k, 7.0 * k, g);
                        } else {
                            p.path(
                                &[
                                    PathSeg::M(cx - 3.0 * k, cy - 3.0 * k),
                                    PathSeg::L(cx + 3.0 * k, cy + 3.0 * k),
                                    PathSeg::M(cx + 3.0 * k, cy - 3.0 * k),
                                    PathSeg::L(cx - 3.0 * k, cy + 3.0 * k),
                                ],
                                None,
                                Some((Rgba(0xC4, 0x2B, 0x1C, 255), 1.5 * k)),
                            );
                        }
                    }
                }
            }
        }
        p.pop_clip();
        if let Some((track, thumb)) = self.scroll_bar() {
            p.fill_rect(
                track[0],
                track[1],
                track[2],
                track[3],
                Rgba(0xE8, 0xE8, 0xE8, 255),
            );
            p.fill_rect(
                thumb[0] + 2.0,
                thumb[1],
                thumb[2] - 4.0,
                thumb[3],
                Rgba(0xA6, 0xA6, 0xA6, 255),
            );
        }
    }

    fn color_of(&self, id: &str) -> u32 {
        let v = &self.vals;
        match id {
            "fillColor" => v.fill_color,
            "lineColor" => v.line_color,
            "stopColor" => v.stops.get(v.stop).map_or(0, |s| s.1),
            "patFg" => self.pattern.map_or(0x44_72C4, |p| p.1),
            "patBg" => self.pattern.map_or(0xFF_FFFF, |p| p.2),
            "shadowColor" => v.shadow_color,
            "glowColor" => v.glow_color,
            "tfillColor" => v.text_fill.unwrap_or(0),
            "tlineColor" => v.text_line.unwrap_or(0),
            _ => 0,
        }
    }

    /// The gradient's colour at `pos` %.
    fn color_at(&self, pos: f64) -> u32 {
        let s = &self.vals.stops;
        let Some(first) = s.first() else {
            return 0xFF_FFFF;
        };
        if pos <= first.0 {
            return first.1;
        }
        for w in s.windows(2) {
            if pos <= w[1].0 {
                let f = if (w[1].0 - w[0].0).abs() < f64::EPSILON {
                    0.0
                } else {
                    (pos - w[0].0) / (w[1].0 - w[0].0)
                };
                let mix = |sh: u32| {
                    let a = f64::from((w[0].1 >> sh) & 0xFF);
                    let b = f64::from((w[1].1 >> sh) & 0xFF);
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    let c = (a + (b - a) * f).round() as u32;
                    c << sh
                };
                return mix(16) | mix(8) | mix(0);
            }
        }
        s.last().map_or(0xFF_FFFF, |l| l.1)
    }

    fn choice_text(&self, id: &str, t: &dyn Fn(&str, &str) -> String) -> String {
        let v = &self.vals;
        match id {
            "gradKind" => {
                let (k, en) = GRAD_KINDS[v.grad_kind.min(3)];
                t(k, en)
            }
            "dash" => words(DASHES[v.dash.min(DASHES.len() - 1)]),
            "anchor" => {
                let (_, k, en) = ANCHORS[v.anchor.min(2)];
                t(k, en)
            }
            "bevel" => v
                .bevel
                .as_ref()
                .map_or_else(|| t("doc21.fpNone", "No effect"), |b| words(&b.0)),
            "rotation" => v
                .camera
                .as_deref()
                .map_or_else(|| t("doc21.fpNone", "No effect"), words),
            _ => String::new(),
        }
    }
}

/// A preset name in words (`outerBottomRight` -> "Outer bottom right"), as round 19's menus.
fn words(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() || (ch.is_ascii_digit() && i > 0) {
            out.push(' ');
            out.extend(ch.to_lowercase());
        } else if i == 0 {
            out.extend(ch.to_uppercase());
        } else {
            out.push(ch);
        }
    }
    out
}

/// Outline of a gradient stop marker: a pentagon pointing down, `top` its upper edge.
fn stop_marker(x: f32, top: f32, k: f32) -> [PathSeg; 6] {
    [
        PathSeg::M(x - 5.0 * k, top + 4.0 * k),
        PathSeg::L(x, top),
        PathSeg::L(x + 5.0 * k, top + 4.0 * k),
        PathSeg::L(x + 5.0 * k, top + 14.0 * k),
        PathSeg::L(x - 5.0 * k, top + 14.0 * k),
        PathSeg::Z,
    ]
}

/// `c` lighter (`pct` > 0, towards white) or darker (towards black).
fn brighten(c: u32, pct: f64) -> u32 {
    let f = (pct / 100.0).clamp(-1.0, 1.0);
    let ch = |sh: u32| {
        let v = f64::from((c >> sh) & 0xFF);
        let n = if f >= 0.0 {
            v + (255.0 - v) * f
        } else {
            v * (1.0 + f)
        };
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let n = n.round().clamp(0.0, 255.0) as u32;
        n << sh
    };
    ch(16) | ch(8) | ch(0)
}

/// An 8 × 8 two-colour bitmap of a pattern (`rows` bits set = foreground).
fn pattern_bmp(rows: [u8; 8], fg: u32, bg: u32) -> Vec<u8> {
    let mut b = b"BM".to_vec();
    let size: u32 = 14 + 40 + 8 + 8 * 4;
    b.extend_from_slice(&size.to_le_bytes());
    b.extend_from_slice(&[0; 4]);
    b.extend_from_slice(&(14u32 + 40 + 8).to_le_bytes());
    b.extend_from_slice(&40u32.to_le_bytes());
    b.extend_from_slice(&8i32.to_le_bytes());
    b.extend_from_slice(&8i32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&[0; 24]);
    #[allow(clippy::cast_possible_truncation)]
    let bgra = |c: u32| [c as u8, (c >> 8) as u8, (c >> 16) as u8, 0];
    b.extend_from_slice(&bgra(bg));
    b.extend_from_slice(&bgra(fg));
    for r in rows.iter().rev() {
        b.extend_from_slice(&[*r, 0, 0, 0]);
    }
    b
}

fn rgb_of(c: bo_model::Color) -> u32 {
    (u32::from(c.r) << 16) | (u32::from(c.g) << 8) | u32::from(c.b)
}

fn rgba(c: u32) -> Rgba {
    #[allow(clippy::cast_possible_truncation)]
    Rgba((c >> 16) as u8, (c >> 8) as u8, c as u8, 255)
}

/// A number without trailing zeros (at most two decimals).
pub(crate) fn fmt(x: f64) -> String {
    let s = format!("{:.2}", (x * 100.0).round() / 100.0);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".into()
    } else {
        s.to_owned()
    }
}

fn circle(
    p: &mut dyn Painter,
    cx: f32,
    cy: f32,
    r: f32,
    fill: Option<Rgba>,
    stroke: Option<(Rgba, f32)>,
) {
    let k = 0.552_284_8 * r;
    p.path(
        &[
            PathSeg::M(cx + r, cy),
            PathSeg::C(cx + r, cy + k, cx + k, cy + r, cx, cy + r),
            PathSeg::C(cx - k, cy + r, cx - r, cy + k, cx - r, cy),
            PathSeg::C(cx - r, cy - k, cx - k, cy - r, cx, cy - r),
            PathSeg::C(cx + k, cy - r, cx + r, cy - k, cx + r, cy),
            PathSeg::Z,
        ],
        fill,
        stroke,
    );
}

/// The page icons (BlueOffice's own drawings: a paint bucket, a pentagon, a box with
/// arrows, a picture).
fn page_icon(p: &mut dyn Painter, pg: Page, r: [f32; 4], k: f32, accent: Rgba) {
    let ink = Rgba(0x44, 0x44, 0x44, 255);
    let (cx, cy) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    let s = 9.0 * k;
    match pg {
        Page::FillLine => {
            p.path(
                &[
                    PathSeg::M(cx - s, cy - s * 0.2),
                    PathSeg::L(cx - s * 0.1, cy - s),
                    PathSeg::L(cx + s * 0.8, cy - s * 0.1),
                    PathSeg::L(cx - s * 0.1, cy + s * 0.8),
                    PathSeg::Z,
                ],
                Some(Rgba(0xFF, 0xFF, 0xFF, 255)),
                Some((ink, 1.2 * k)),
            );
            p.path(
                &[
                    PathSeg::M(cx + s * 0.8, cy + s * 0.2),
                    PathSeg::Q(cx + s * 1.1, cy + s * 0.7, cx + s * 0.8, cy + s * 0.9),
                    PathSeg::Q(cx + s * 0.5, cy + s * 0.7, cx + s * 0.8, cy + s * 0.2),
                ],
                Some(accent),
                None,
            );
        }
        Page::Effects => {
            let pts: Vec<(f32, f32)> = (0..5)
                .map(|i| {
                    let a = std::f32::consts::PI * (-0.5 + 0.4 * i as f32);
                    (cx + s * a.cos(), cy + s * a.sin())
                })
                .collect();
            let mut segs = vec![PathSeg::M(pts[0].0, pts[0].1)];
            segs.extend(pts[1..].iter().map(|q| PathSeg::L(q.0, q.1)));
            segs.push(PathSeg::Z);
            p.path(
                &segs,
                Some(Rgba(0xE8, 0xF0, 0xFA, 255)),
                Some((ink, 1.2 * k)),
            );
        }
        Page::Layout => {
            p.path(
                &[
                    PathSeg::M(cx - s * 0.7, cy - s),
                    PathSeg::L(cx + s, cy - s),
                    PathSeg::L(cx + s, cy + s),
                    PathSeg::L(cx - s * 0.7, cy + s),
                    PathSeg::Z,
                ],
                None,
                Some((ink, 1.2 * k)),
            );
            p.fill_rect(cx - s, cy - s, 1.2 * k, 2.0 * s, ink);
            p.fill_rect(cx - s * 0.3, cy - 0.6 * k, s, 1.2 * k, accent);
            p.fill_rect(cx + s * 0.1, cy - s * 0.4, 1.2 * k, s * 0.8, accent);
        }
        Page::Picture => {
            p.path(
                &[
                    PathSeg::M(cx - s, cy - s * 0.8),
                    PathSeg::L(cx + s, cy - s * 0.8),
                    PathSeg::L(cx + s, cy + s * 0.8),
                    PathSeg::L(cx - s, cy + s * 0.8),
                    PathSeg::Z,
                ],
                Some(Rgba(0xFF, 0xFF, 0xFF, 255)),
                Some((ink, 1.2 * k)),
            );
            p.path(
                &[
                    PathSeg::M(cx - s * 0.8, cy + s * 0.6),
                    PathSeg::L(cx - s * 0.2, cy - s * 0.1),
                    PathSeg::L(cx + s * 0.2, cy + s * 0.3),
                    PathSeg::L(cx + s * 0.5, cy),
                    PathSeg::L(cx + s * 0.8, cy + s * 0.6),
                    PathSeg::Z,
                ],
                Some(accent),
                None,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spins_build_custom_effects_and_gradients() {
        let mut p = FmtPane::new(Target::Shape, Page::Effects, Some("shadow"));
        let c = p.set_spin("shBlur", 9.0);
        assert_eq!(c[0]["cmd"], "shapeEffect");
        assert!(c[0]["preset"]
            .as_str()
            .is_some_and(|s| s.contains("blur=9")));
        let c = p.set_color("glowColor", 0xFF_0000);
        assert!(c[0]["preset"]
            .as_str()
            .is_some_and(|s| s.contains("FF0000")));
        p.page = Page::FillLine;
        p.vals.fill = 2;
        let c = p.set_spin("stopPos", 30.0);
        assert_eq!(c[0]["cmd"], "shapeGradient");
        assert_eq!(c[0]["stops"][0]["pos"], 30.0);
        assert_eq!(fmt(7.20), "7.2");
        assert_eq!(fmt(-0.0), "0");
        let pic = FmtPane::new(Target::Picture, Page::Picture, Some("corr"));
        assert!(pic
            .rows()
            .iter()
            .any(|r| matches!(r.kind, RowKind::Spin("fxBright", ..))));
    }

    struct Null;
    impl crate::Painter for Null {
        fn clear(&mut self, _: Rgba) {}
        fn fill_rect(&mut self, _: f32, _: f32, _: f32, _: f32, _: Rgba) {}
        fn page(&mut self, _: usize, _: i32, _: i32) {}
        fn text(&mut self, _: &str, _: f32, _: f32, _: TextStyle, _: Rgba) {}
        fn path(&mut self, _: &[PathSeg], _: Option<Rgba>, _: Option<(Rgba, f32)>) {}
        fn push_clip(&mut self, _: f32, _: f32, _: f32, _: f32) {}
        fn pop_clip(&mut self) {}
    }

    #[test]
    fn every_page_paints_with_every_section_open() {
        for target in [Target::Shape, Target::Picture] {
            for text in [false, true] {
                let mut p = FmtPane::new(target, Page::FillLine, None);
                p.text_tab = text;
                p.layout([1000.0, 100.0, width(1.25), 900.0], 1.25);
                p.vals.shadow = Some([60.0, 100.0, 4.0, 45.0, 3.0, 0.0]);
                p.vals.reflection = Some([48.0, 55.0, 0.5, 0.0]);
                p.vals.glow = Some([8.0, 60.0]);
                p.vals.bevel = Some(("circle".into(), 6.0, 6.0));
                for page in p.pages() {
                    p.page = page;
                    for fill in 0..4 {
                        p.vals.fill = fill;
                        for r in p.rows() {
                            if let RowKind::Section(id, ..) = r.kind {
                                p.open.insert(id);
                            }
                        }
                        p.paint(&mut Null, &|_, en| en.to_owned());
                        // Every row's controls answer a press.
                        let top = p.content_top();
                        for r in p.rows() {
                            let b = p.box_rect(&r);
                            let _ = p.press(b[0] + 2.0, top + r.y * 1.25 + 4.0);
                            let _ = p.press(b[0] + b[2] - 3.0, b[1] + 3.0);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn presses_open_sections_and_run_commands() {
        let mut p = FmtPane::new(Target::Shape, Page::FillLine, None);
        p.layout([1000.0, 100.0, width(1.0), 800.0], 1.0);
        let top = p.content_top();
        // Open Fill, then choose Gradient fill (the third option).
        assert_eq!(p.press(1010.0, top + 5.0), PaneAct::Redraw);
        let rows = p.rows();
        let grad = rows
            .iter()
            .find(|r| matches!(r.kind, RowKind::Radio("fill", 2, ..)))
            .expect("gradient option");
        let act = p.press(1040.0, top + grad.y + 5.0);
        assert!(matches!(act, PaneAct::Run(ref c) if c[0]["cmd"] == "shapeGradient"));
        // The add-stop button adds a stop.
        let row = p
            .rows()
            .into_iter()
            .find(|r| r.kind == RowKind::Stops)
            .expect("stops");
        let (_, add, _) = p.stop_rects(&row);
        let n = p.vals.stops.len();
        assert!(matches!(
            p.press(add[0] + 2.0, add[1] + 2.0),
            PaneAct::Run(_)
        ));
        assert_eq!(p.vals.stops.len(), n + 1);
        // Pages and close.
        let (_, r) = p.page_rects()[1];
        p.press(r[0] + 2.0, r[1] + 2.0);
        assert_eq!(p.page, Page::Effects);
        let c = p.close_rect();
        assert_eq!(p.press(c[0] + 2.0, c[1] + 2.0), PaneAct::Close);
        // Menus of the choices.
        assert!(p.menu("shadowPresets").len() > 10);
        assert_eq!(p.choose("fp21.p:shadow=outerBottom").len(), 1);
        assert_eq!(p.choose("fp21.anchor:1")[0]["anchor"], "ctr");
    }
}
