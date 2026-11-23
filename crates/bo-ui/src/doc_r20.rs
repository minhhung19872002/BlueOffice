//! BlueDoc parity round 20 (docs/ui/REPORT.md "Vòng 110"): Shape Format — Shape Fill ▾ with
//! More Fill Colors…, Picture…, Gradient ▸ (light and dark variations, More Gradients…) and
//! Texture ▸ (BlueOffice's own textures, More Textures…), Shape Outline ▾ with More Outline
//! Colors…, Shape Effects ▾ (Preset, Shadow, Reflection, Glow, Soft Edges, Bevel, 3-D
//! Rotation — the Picture Effects presets of round 19 on the shape) and the WordArt Styles
//! group (Quick Styles, Text Fill ▾, Text Outline ▾, Text Effects ▾). The commands run on the
//! engine (`bo_engine::doc_shapestyle`). A child module of the shell.
use super::DocWindow;
use crate::doc_forms::DocFormKind;
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::RibbonHit;
use bo_engine::doc_picstyle as ps;
use bo_engine::doc_shapestyle as ss;
use serde_json::{json, Value};

#[cfg(test)]
#[path = "doc_r20_tests.rs"]
mod tests;

/// Round-20 state of the window.
#[derive(Default)]
pub(super) struct R20 {
    /// More Colors… was opened for the shape's `fill`, `line` or `text` (the Colors dialog
    /// answers through Home › Font Color's path, `r16.fontColor:`).
    more: Option<&'static str>,
    /// Fill Effects is open for the selected shape (its result fills the shape).
    shape_dialog: bool,
    /// Shape Fill › Picture… waits for the host's picture.
    picture: bool,
}

/// Standard colours (hex, key, English): the round-18 / round-19 keys.
const COLORS: [(&str, &str, &str); 12] = [
    ("000000", "doc19.cBlack", "Black"),
    ("FFFFFF", "doc19.cWhite", "White"),
    ("C00000", "doc18.cDarkRed", "Dark Red"),
    ("FF0000", "doc18.cRed", "Red"),
    ("FFC000", "doc18.cOrange", "Orange"),
    ("FFFF00", "doc18.cYellow", "Yellow"),
    ("92D050", "doc18.cLightGreen", "Light Green"),
    ("00B050", "doc18.cGreen", "Green"),
    ("00B0F0", "doc18.cLightBlue", "Light Blue"),
    ("0070C0", "doc18.cBlue", "Blue"),
    ("002060", "doc18.cDarkBlue", "Dark Blue"),
    ("7030A0", "doc18.cPurple", "Purple"),
];

/// Names of the nine gradient directions of [`ss::DIRECTIONS`] (key, English).
const DIRECTIONS: [(&str, &str); 9] = [
    ("doc20.dTlBr", "Diagonal, top left to bottom right"),
    ("doc20.dDown", "Downwards"),
    ("doc20.dTrBl", "Diagonal, top right to bottom left"),
    ("doc20.dRight", "To the right"),
    ("doc20.dCenter", "Out from the centre"),
    ("doc20.dLeft", "To the left"),
    ("doc20.dBlTr", "Diagonal, bottom left to top right"),
    ("doc20.dUp", "Upwards"),
    ("doc20.dBrTl", "Diagonal, bottom right to top left"),
];

/// Text Fill › Gradient: (from, to, angle).
const TEXT_GRADIENTS: [(&str, &str, f32); 6] = [
    ("4472C4", "1F3864", 90.0),
    ("ED7D31", "C00000", 90.0),
    ("FFC000", "ED7D31", 90.0),
    ("70AD47", "375623", 90.0),
    ("5B9BD5", "7030A0", 0.0),
    ("A5A5A5", "262626", 90.0),
];

/// Weights (pt) of Text Outline.
const WEIGHTS: [&str; 7] = ["0.25", "0.5", "0.75", "1", "1.5", "2.25", "3"];

/// The Picture Effects categories of round 19 offered by Shape Effects (kind, key, English).
const KINDS: [(&str, &str, &str); 7] = [
    ("preset", "doc19.kPreset", "Preset"),
    ("shadow", "doc19.kShadow", "Shadow"),
    ("reflection", "doc19.kReflection", "Reflection"),
    ("glow", "doc19.kGlow", "Glow"),
    ("softEdge", "doc19.kSoftEdges", "Soft Edges"),
    ("bevel", "doc19.kBevel", "Bevel"),
    ("rotation", "doc19.kRotation", "3-D Rotation"),
];

/// Commands of round 20 (they work on a selected shape).
pub(crate) const IDS: [&str; 8] = [
    "shape.alignText",
    "shape.fill",
    "shape.outline",
    "shape.effects",
    "shape.wordArt",
    "shape.textFill",
    "shape.textOutline",
    "shape.textEffects",
];

/// Words of a camel-case name ("offsetBottomRight" → "Offset Bottom Right").
fn words(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if i == 0 {
            out.extend(c.to_uppercase());
        } else if c.is_ascii_uppercase()
            || (c.is_ascii_digit() && !out.ends_with(|d: char| d.is_ascii_digit()))
        {
            out.push(' ');
            out.push(c);
        } else {
            out.push(c);
        }
    }
    out
}

fn b64(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// A shape command for the `setPageFill` command of Fill Effects (opened for a shape): a
/// gradient with its stops and direction, a tiled texture or pattern, a stretched picture.
pub(crate) fn shape_fill_of(page: &Value) -> Option<Value> {
    let fill = &page["fill"];
    let hex = |v: &Value| -> Option<String> {
        if let Some(s) = v.as_str() {
            return Some(s.to_owned());
        }
        let n = |k: &str| v[k].as_u64().map(|x| x.min(255));
        Some(format!("{:02X}{:02X}{:02X}", n("r")?, n("g")?, n("b")?))
    };
    match fill["kind"].as_str()? {
        k @ ("gradient" | "gradientRadial") => {
            let c1 = page["color"].as_str().unwrap_or("FFFFFF").to_owned();
            let mut stops: Vec<(f64, String)> = match fill["stops"].as_array() {
                Some(a) => a
                    .iter()
                    .filter_map(|s| Some((s[0].as_f64()? / 10.0, hex(&s[1])?)))
                    .collect(),
                None => vec![(0.0, c1), (100.0, hex(&fill["color2"])?)],
            };
            // Variants (Part 4, §19.1.2.5 focus): 100 reverses, ±50 mirrors.
            let focus = fill["focus"].as_i64().unwrap_or(0);
            if focus == 100 {
                stops = stops
                    .into_iter()
                    .rev()
                    .map(|(p, c)| (100.0 - p, c))
                    .collect();
            } else if focus.abs() == 50 && stops.len() >= 2 {
                let (a, b) = (stops[0].1.clone(), stops[stops.len() - 1].1.clone());
                let (outer, mid) = if focus > 0 { (a, b) } else { (b, a) };
                stops = vec![(0.0, outer.clone()), (50.0, mid), (100.0, outer)];
            }
            let stops: Vec<Value> = stops
                .into_iter()
                .map(|(p, c)| json!({ "pos": p, "color": c }))
                .collect();
            if k == "gradientRadial" {
                let pos = &fill["focusPosition"];
                let (x, y) = (
                    pos[0].as_f64().unwrap_or(500.0) / 10.0,
                    pos[1].as_f64().unwrap_or(500.0) / 10.0,
                );
                Some(
                    json!({ "cmd": "shapeGradient", "kind": "circle", "stops": stops,
                             "focus": [x, y, 100.0 - x, 100.0 - y] }),
                )
            } else {
                // VML angle 0 = top to bottom; DrawingML lin@ang 90° = top to bottom.
                let angle = 90.0 + fill["angle"].as_f64().unwrap_or(0.0);
                Some(
                    json!({ "cmd": "shapeGradient", "kind": "linear", "angle": angle,
                             "stops": stops }),
                )
            }
        }
        "tile" => Some(json!({ "cmd": "shapePictureFill", "data": page["picture"], "tile": true })),
        "frame" => {
            Some(json!({ "cmd": "shapePictureFill", "data": page["picture"], "tile": false }))
        }
        "pattern" => {
            // The two-colour pattern bitmap in the pattern's own colours, tiled.
            use base64::Engine as _;
            let mut bmp = base64::engine::general_purpose::STANDARD
                .decode(page["picture"].as_str()?)
                .ok()?;
            let fg = hex(&page["color"]).unwrap_or_else(|| "000000".into());
            let bg = hex(&fill["color2"]).unwrap_or_else(|| "FFFFFF".into());
            let bgr = |h: &str| {
                let v = u32::from_str_radix(h, 16).unwrap_or(0);
                [
                    (v & 255) as u8,
                    ((v >> 8) & 255) as u8,
                    ((v >> 16) & 255) as u8,
                    0,
                ]
            };
            if bmp.len() >= 62 {
                bmp[54..58].copy_from_slice(&bgr(&bg));
                bmp[58..62].copy_from_slice(&bgr(&fg));
            }
            Some(json!({ "cmd": "shapePictureFill", "data": b64(&bmp), "tile": true }))
        }
        _ => None,
    }
}

impl DocWindow {
    fn r20_t(&self, key: &str, fallback: &str) -> String {
        let v = self.strings.get(key);
        if v == key {
            fallback.to_owned()
        } else {
            v.to_owned()
        }
    }

    /// A round-20 command; `None` when `id` is not one.
    pub(super) fn r20_action(&mut self, id: &str) -> Option<bool> {
        if id == "design.pageColor.fill" {
            // Fill Effects for the page, not the shape.
            self.r20.shape_dialog = false;
            return None;
        }
        if let Some(hex) = id.strip_prefix("r16.fontColor:") {
            // The Colors dialog answered for the shape (More Fill / Outline Colors…).
            let target = self.r20.more.take()?;
            let hex = hex.to_owned();
            return Some(match target {
                "fill" => self.r20_run(&json!({ "cmd": "pictureShapeFill", "color": hex })),
                "line" => self.r20_run(&json!({ "cmd": "pictureShapeOutline", "color": hex })),
                _ => self.r20_run(&json!({ "cmd": "shapeTextFill", "color": hex })),
            });
        }
        Some(match id {
            _ if IDS.contains(&id) => {
                let sid = IDS.iter().copied().find(|i| *i == id).unwrap_or(IDS[0]);
                self.r20_menu(sid, None)
            }
            _ if id.starts_with("r20.menu:") => {
                let (anchor, sub) = id["r20.menu:".len()..]
                    .split_once('/')
                    .unwrap_or(("shape.fill", ""));
                let anchor = IDS.iter().copied().find(|i| *i == anchor).unwrap_or(IDS[0]);
                let sub = sub.to_owned();
                self.r20_menu(anchor, Some(&sub))
            }
            "r20.moreFill" | "r20.moreLine" | "r20.moreText" => {
                self.r20.more = Some(match id {
                    "r20.moreFill" => "fill",
                    "r20.moreLine" => "line",
                    _ => "text",
                });
                self.r16.font_more = true;
                self.r3_open_form(DocFormKind::Colors)
            }
            "r20.picture" => {
                self.menu = None;
                self.r20.picture = true;
                self.requests.push(super::Request::PickPicture);
                false
            }
            _ if id.starts_with("r20.fillEffects:") => {
                let tab: usize = id["r20.fillEffects:".len()..].parse().unwrap_or(0);
                self.r20_fill_effects(tab)
            }
            _ if id.starts_with("r20.grad:") => {
                let rest = &id["r20.grad:".len()..];
                let (var, i) = rest.split_once(':').unwrap_or((rest, "0"));
                let index: u64 = i.parse().unwrap_or(0);
                self.r20_run(&json!({ "cmd": "shapeGradient", "variation": var, "index": index }))
            }
            _ if id.starts_with("r20.tex:") => {
                let i: usize = id["r20.tex:".len()..].parse().unwrap_or(0);
                match crate::slide_textures22::png(i) {
                    Some(png) => self.r20_run(&json!({
                        "cmd": "shapePictureFill", "data": b64(&png), "tile": true
                    })),
                    None => false,
                }
            }
            _ if id.starts_with("r20.fx:") => {
                let rest = &id["r20.fx:".len()..];
                let (kind, preset) = rest.split_once('=').unwrap_or((rest, "none"));
                self.r20_run(&json!({ "cmd": "shapeEffect", "kind": kind, "preset": preset }))
            }
            _ if id.starts_with("r20.anchor:") => self.r20_run(&json!({
                "cmd": "shapeTextAnchor", "anchor": &id["r20.anchor:".len()..]
            })),
            _ if id.starts_with("r20.wa:") => {
                let i: u64 = id["r20.wa:".len()..].parse().unwrap_or(0);
                self.r20_run(&json!({ "cmd": "wordArtStyle", "index": i }))
            }
            _ if id.starts_with("r20.tf:") => self.r20_run(&json!({
                "cmd": "shapeTextFill", "color": &id["r20.tf:".len()..]
            })),
            _ if id.starts_with("r20.tg:") => {
                let i: usize = id["r20.tg:".len()..].parse().unwrap_or(0);
                let (a, b, angle) = TEXT_GRADIENTS[i.min(TEXT_GRADIENTS.len() - 1)];
                self.r20_run(&json!({ "cmd": "shapeTextFill", "gradient": [a, b, angle] }))
            }
            _ if id.starts_with("r20.to:") => self.r20_run(&json!({
                "cmd": "shapeTextOutline", "color": &id["r20.to:".len()..]
            })),
            _ if id.starts_with("r20.tw:") => {
                let w: f64 = id["r20.tw:".len()..].parse().unwrap_or(0.75);
                let color = self.r20_text_outline_color();
                self.r20_run(&json!({ "cmd": "shapeTextOutline", "color": color, "weight": w }))
            }
            _ if id.starts_with("r20.te:") => {
                let rest = &id["r20.te:".len()..];
                let (kind, preset) = rest.split_once('=').unwrap_or((rest, "none"));
                self.r20_run(&json!({ "cmd": "shapeTextEffect", "kind": kind, "preset": preset }))
            }
            _ => return None,
        })
    }

    /// File › Options › Customize Ribbon: show the main tabs except `hidden` (label keys).
    pub(super) fn r20_apply_ribbon(&mut self, hidden: &[String]) {
        let tabs: Vec<crate::ribbon::Tab> = crate::ribbon::doc_tabs()
            .into_iter()
            .filter(|t| !hidden.iter().any(|h| h == t.label))
            .collect();
        let now = self.ribbon.fixed_labels();
        if tabs.is_empty() || tabs.iter().map(|t| t.label).eq(now.iter().copied()) {
            return;
        }
        self.ribbon.set_fixed_tabs(tabs);
        self.relayout_pending = true;
    }

    /// A picture the host read for Shape Fill › Picture…; `None` when none was asked for.
    pub(super) fn r20_take_picture(&mut self, bytes: &[u8]) -> Option<bool> {
        if !std::mem::take(&mut self.r20.picture) {
            return None;
        }
        Some(self.r20_run(&json!({
            "cmd": "shapePictureFill", "data": b64(bytes), "tile": false
        })))
    }

    /// The commands of a closing dialog: Fill Effects opened for a shape fills the shape.
    pub(super) fn r20_translate(&mut self, cmds: Vec<Value>) -> Vec<Value> {
        if !self.r20.shape_dialog || !cmds.iter().any(|c| c["cmd"] == "setPageFill") {
            return cmds;
        }
        self.r20.shape_dialog = false;
        cmds.into_iter()
            .filter_map(|c| {
                if c["cmd"] == "setPageFill" {
                    shape_fill_of(&c)
                } else {
                    Some(c)
                }
            })
            .collect()
    }

    /// The text outline colour of the selected shape's text (Text Outline › Weight keeps it).
    fn r20_text_outline_color(&self) -> String {
        let img = self.r20_shape();
        img.and_then(|i| i.shape)
            .and_then(|s| s.text)
            .and_then(|b| {
                b.iter().find_map(|b| match b {
                    bo_model::Block::Paragraph(p) => p.runs.iter().find_map(|r| {
                        r.props
                            .effects
                            .as_ref()
                            .and_then(|e| e.outline.as_ref())
                            .and_then(|o| match &o.fill {
                                bo_model::run_fx::FxFill::Solid { color } => {
                                    Some(color.rgb.to_hex())
                                }
                                _ => None,
                            })
                    }),
                    _ => None,
                })
            })
            .unwrap_or_else(|| "000000".to_owned())
    }

    /// The selected shape.
    fn r20_shape(&self) -> Option<bo_model::Image> {
        self.view
            .engine()
            .session()
            .and_then(bo_edit::Session::tools_shape)
    }

    /// Run a Shape Format command on the selected shape.
    fn r20_run(&mut self, v: &Value) -> bool {
        self.menu = None;
        let ok = self.view.picture_command(v);
        self.sync_context();
        ok
    }

    /// Fill Effects (Gradient, Texture, Pattern, Picture tabs) for the selected shape, on tab
    /// `tab`: More Gradients… / More Textures….
    fn r20_fill_effects(&mut self, tab: usize) -> bool {
        self.menu = None;
        let color = self
            .r20_shape()
            .and_then(|i| i.shape)
            .and_then(|s| s.fill)
            .map_or_else(|| "4472C4".to_owned(), bo_model::Color::to_hex);
        let mut c = self.form_context();
        c.extra["r18"] = json!({ "color": color });
        let mut f = crate::doc_forms::open(DocFormKind::FillEffects, &self.strings, &c);
        f.select_tab_for_r20(tab);
        self.r3.form = Some(f);
        self.r20.shape_dialog = true;
        self.relayout_pending = true;
        true
    }

    /// Open the drop-down of `id` (`sub`: a second-level menu at the same button).
    #[allow(clippy::too_many_lines)]
    fn r20_menu(&mut self, id: &'static str, sub: Option<&str>) -> bool {
        let img = self.r20_shape();
        let shape = img.as_ref().and_then(|i| i.shape.as_deref());
        let style = shape.and_then(|s| s.style.as_deref());
        let fx = style.and_then(|s| s.effects.clone()).unwrap_or_default();
        let item = |label: String, cmd: String, checked: bool| MenuItem {
            label,
            choice: Choice::Command(cmd),
            checked,
            dim: false,
        };
        let sub_item = |label: String, menu: String| MenuItem {
            label: format!("{label} \u{25B8}"),
            choice: Choice::Command(format!("r20.menu:{id}/{menu}")),
            checked: false,
            dim: false,
        };
        let fill_hex = shape.and_then(|s| s.fill).map(bo_model::Color::to_hex);
        let mut items = Vec::new();
        match (id, sub) {
            ("shape.fill", None) => {
                let plain = style.is_none_or(|s| s.gradient.is_none() && s.picture.is_none());
                items.push(item(
                    self.r20_t("doc18.noFill", "No Fill"),
                    "r18.fill:none".into(),
                    fill_hex.is_none(),
                ));
                for (hex, key, en) in COLORS {
                    let on = plain && fill_hex.as_deref() == Some(hex);
                    items.push(item(self.r20_t(key, en), format!("r18.fill:{hex}"), on));
                }
                items.push(item(
                    self.r20_t("doc20.moreFill", "More Fill Colors..."),
                    "r20.moreFill".into(),
                    false,
                ));
                items.push(item(
                    self.r20_t("doc20.picture", "Picture..."),
                    "r20.picture".into(),
                    style.is_some_and(|s| s.picture.as_ref().is_some_and(|p| !p.tile)),
                ));
                items.push(sub_item(
                    self.r20_t("doc20.gradient", "Gradient"),
                    "gradient".into(),
                ));
                items.push(sub_item(
                    self.r20_t("doc20.texture", "Texture"),
                    "texture".into(),
                ));
            }
            ("shape.fill", Some("gradient")) => {
                items.push(item(
                    self.r20_t("doc20.noGradient", "No Gradient"),
                    "r20.grad:none".into(),
                    style.is_none_or(|s| s.gradient.is_none()),
                ));
                for (var, key, en) in [
                    ("light", "doc20.light", "Light"),
                    ("dark", "doc20.dark", "Dark"),
                ] {
                    let base = self.r20_t(key, en);
                    for (i, (dk, den)) in DIRECTIONS.iter().enumerate() {
                        items.push(item(
                            format!("{base}: {}", self.r20_t(dk, den)),
                            format!("r20.grad:{var}:{i}"),
                            false,
                        ));
                    }
                }
                items.push(item(
                    self.r20_t("doc20.moreGradients", "More Gradients..."),
                    "r20.fillEffects:0".into(),
                    false,
                ));
            }
            ("shape.fill", Some(_)) => {
                for (i, name) in crate::slide_textures22::NAMES.iter().enumerate() {
                    let key = format!("doc20.tex{i}");
                    items.push(item(self.r20_t(&key, name), format!("r20.tex:{i}"), false));
                }
                items.push(item(
                    self.r20_t("doc20.moreTextures", "More Textures..."),
                    "r20.fillEffects:1".into(),
                    false,
                ));
            }
            ("shape.outline", _) => {
                let line = shape.and_then(|s| s.line.as_ref());
                items.push(item(
                    self.r20_t("doc18.noOutline", "No Outline"),
                    "r18.olc:none".into(),
                    line.is_none(),
                ));
                for (hex, key, en) in COLORS {
                    let on = line.is_some_and(|l| l.color.to_hex() == hex);
                    items.push(item(self.r20_t(key, en), format!("r18.olc:{hex}"), on));
                }
                items.push(item(
                    self.r20_t("doc20.moreLine", "More Outline Colors..."),
                    "r20.moreLine".into(),
                    false,
                ));
                for w in ["0.25", "0.5", "0.75", "1", "1.5", "2.25", "3", "4.5", "6"] {
                    let on = line.is_some_and(|l| {
                        (f64::from(l.width.to_pt()) - w.parse::<f64>().unwrap_or(0.0)).abs() < 0.01
                    });
                    items.push(item(format!("{w} pt"), format!("r18.olw:{w}"), on));
                }
                for (dash, key, en) in [
                    ("solid", "doc18.dSolid", "Solid"),
                    ("sysDot", "doc18.dRoundDot", "Round Dot"),
                    ("sysDash", "doc18.dSquareDot", "Square Dot"),
                    ("dash", "doc18.dDash", "Dash"),
                    ("dashDot", "doc18.dDashDot", "Dash Dot"),
                    ("lgDash", "doc18.dLongDash", "Long Dash"),
                ] {
                    let on = line.is_some_and(|l| l.dash.as_deref().unwrap_or("solid") == dash);
                    items.push(item(self.r20_t(key, en), format!("r18.old:{dash}"), on));
                }
            }
            ("shape.effects", None) => {
                for (k, key, en) in KINDS {
                    let on = match k {
                        "shadow" => fx.shadow.is_some(),
                        "reflection" => fx.reflection.is_some(),
                        "glow" => fx.glow.is_some(),
                        "softEdge" => fx.soft_edge.is_some(),
                        "bevel" => fx.bevel.is_some(),
                        "rotation" => fx.camera.is_some(),
                        _ => false,
                    };
                    items.push(MenuItem {
                        checked: on,
                        ..sub_item(self.r20_t(key, en), k.to_owned())
                    });
                }
            }
            ("shape.effects", Some(k)) => {
                let none = match k {
                    "preset" => self.r20_t("doc19.noPreset", "No Presets"),
                    "shadow" => self.r20_t("doc19.noShadow", "No Shadow"),
                    "reflection" => self.r20_t("doc19.noReflection", "No Reflection"),
                    "glow" => self.r20_t("doc19.noGlow", "No Glow"),
                    "softEdge" => self.r20_t("doc19.noSoftEdges", "No Soft Edges"),
                    "bevel" => self.r20_t("doc19.noBevel", "No Bevel"),
                    _ => self.r20_t("doc19.noRotation", "No Rotation"),
                };
                items.push(item(none, format!("r20.fx:{k}=none"), false));
                let mut add = |label: String, preset: String, on: bool| {
                    items.push(item(label, format!("r20.fx:{k}={preset}"), on));
                };
                match k {
                    "preset" => {
                        for i in 0..ps::PRESETS.len() {
                            add(
                                format!("{} {}", self.r20_t("doc19.kPreset", "Preset"), i + 1),
                                i.to_string(),
                                false,
                            );
                        }
                    }
                    "shadow" => {
                        for (name, ..) in ps::SHADOWS {
                            add(words(name), name.to_owned(), false);
                        }
                    }
                    "reflection" => {
                        for (name, ..) in ps::REFLECTIONS {
                            add(words(name), name.to_owned(), false);
                        }
                    }
                    "glow" => {
                        for size in ps::GLOW_SIZES {
                            for c in ps::GLOW_COLORS {
                                let on = fx.glow.as_ref().is_some_and(|g| {
                                    g.color == c && (g.rad as f64 / 12_700.0 - size).abs() < 0.01
                                });
                                add(
                                    format!("{size} pt, #{c:06X}"),
                                    format!("{size}:{c:06X}"),
                                    on,
                                );
                            }
                        }
                    }
                    "softEdge" => {
                        for r in ps::SOFT_EDGES {
                            let on = fx
                                .soft_edge
                                .is_some_and(|e| (e as f64 / 12_700.0 - r).abs() < 0.01);
                            add(format!("{r} pt"), r.to_string(), on);
                        }
                    }
                    "bevel" => {
                        for b in ps::BEVELS {
                            let on = fx.bevel.as_ref().is_some_and(|x| x.prst == b);
                            add(words(b), b.to_owned(), on);
                        }
                    }
                    _ => {
                        for c in ps::ROTATIONS {
                            add(words(c), c.to_owned(), fx.camera.as_deref() == Some(c));
                        }
                    }
                }
            }
            ("shape.wordArt", _) => {
                for i in 0..ss::WORDART.len() {
                    items.push(item(
                        format!(
                            "{} {}",
                            self.r20_t("doc20.wordArtStyle", "WordArt Style"),
                            i + 1
                        ),
                        format!("r20.wa:{i}"),
                        false,
                    ));
                }
            }
            ("shape.textFill", None) => {
                items.push(item(
                    self.r20_t("doc18.noFill", "No Fill"),
                    "r20.tf:none".into(),
                    false,
                ));
                for (hex, key, en) in COLORS {
                    items.push(item(self.r20_t(key, en), format!("r20.tf:{hex}"), false));
                }
                items.push(item(
                    self.r20_t("doc20.moreFill", "More Fill Colors..."),
                    "r20.moreText".into(),
                    false,
                ));
                items.push(sub_item(
                    self.r20_t("doc20.gradient", "Gradient"),
                    "gradient".into(),
                ));
            }
            ("shape.textFill", Some(_)) => {
                for (i, (a, b, _)) in TEXT_GRADIENTS.iter().enumerate() {
                    items.push(item(
                        format!("#{a} \u{2192} #{b}"),
                        format!("r20.tg:{i}"),
                        false,
                    ));
                }
            }
            ("shape.textOutline", _) => {
                items.push(item(
                    self.r20_t("doc18.noOutline", "No Outline"),
                    "r20.to:none".into(),
                    false,
                ));
                for (hex, key, en) in COLORS {
                    items.push(item(self.r20_t(key, en), format!("r20.to:{hex}"), false));
                }
                for w in WEIGHTS {
                    items.push(item(format!("{w} pt"), format!("r20.tw:{w}"), false));
                }
            }
            ("shape.textEffects", None) => {
                for (k, key, en) in [
                    ("shadow", "doc19.kShadow", "Shadow"),
                    ("reflection", "doc19.kReflection", "Reflection"),
                    ("glow", "doc19.kGlow", "Glow"),
                ] {
                    items.push(sub_item(self.r20_t(key, en), k.to_owned()));
                }
                items.push(item(
                    self.r20_t("doc20.noTextEffects", "No Text Effects"),
                    "r20.te:none=none".into(),
                    false,
                ));
            }
            ("shape.alignText", _) => {
                let a = shape.map(|s| s.anchor);
                for (v, key, en, on) in [
                    (
                        "t",
                        "doc20.top",
                        "Top",
                        a == Some(bo_model::TextAnchor::Top),
                    ),
                    (
                        "ctr",
                        "doc20.middle",
                        "Middle",
                        a == Some(bo_model::TextAnchor::Center),
                    ),
                    (
                        "b",
                        "doc20.bottom",
                        "Bottom",
                        a == Some(bo_model::TextAnchor::Bottom),
                    ),
                ] {
                    items.push(item(self.r20_t(key, en), format!("r20.anchor:{v}"), on));
                }
            }
            ("shape.textEffects", Some(k)) => {
                let none = match k {
                    "shadow" => self.r20_t("doc19.noShadow", "No Shadow"),
                    "reflection" => self.r20_t("doc19.noReflection", "No Reflection"),
                    _ => self.r20_t("doc19.noGlow", "No Glow"),
                };
                items.push(item(none, format!("r20.te:{k}=none"), false));
                match k {
                    "shadow" => {
                        for (name, ..) in ss::TEXT_SHADOWS {
                            items.push(item(words(name), format!("r20.te:shadow={name}"), false));
                        }
                    }
                    "reflection" => {
                        for (name, ..) in ss::TEXT_REFLECTIONS {
                            items.push(item(
                                words(name),
                                format!("r20.te:reflection={name}"),
                                false,
                            ));
                        }
                    }
                    _ => {
                        for (i, (r, c, _)) in ss::TEXT_GLOWS.iter().enumerate() {
                            items.push(item(
                                format!("{r} pt, #{c}"),
                                format!("r20.te:glow={i}"),
                                false,
                            ));
                        }
                    }
                }
            }
            _ => {}
        }
        let anchor = self.ribbon.rect_of(RibbonHit::Command(id)).unwrap_or([
            self.size.0 / 2.0,
            120.0,
            60.0,
            30.0,
        ]);
        self.menu = Some(Menu::open(items, anchor, self.size, self.scale));
        true
    }
}

/// Labels and icons of the WordArt Styles buttons (BlueOffice's own drawings: a letter A
/// with a colour bar, an outlined A, a glowing A).
pub(crate) fn install(icons: &mut crate::icons::Icons, s: &mut crate::strings::Strings) {
    use bo_render::Rgba;
    for (id, key, en) in [
        ("shape.textFill", "doc20.textFill", "Text Fill"),
        ("shape.textOutline", "doc20.textOutline", "Text Outline"),
        ("shape.textEffects", "doc20.textEffects", "Text Effects"),
        ("shape.wordArt", "doc20.quickStyles", "Quick Styles"),
        ("shape.alignText", "doc20.alignText", "Align Text"),
    ] {
        let v = s.get(key).to_owned();
        s.set(
            &format!("commands.{id}"),
            if v == key { en } else { v.as_str() },
        );
    }
    if s.get("doc20.groupText") == "doc20.groupText" {
        s.set("doc20.groupText", "Text");
    }
    let rgb = |r, g, b| Some(Rgba(r, g, b, 255));
    // Align Text: three lines over a two-headed vertical arrow.
    icons.add_solid(
        "shape.alignText",
        &[
            (
                "M9 4h12v2h-12ZM9 11h12v2h-12ZM9 18h12v2h-12Z",
                rgb(0x26, 0x26, 0x26),
            ),
            (
                "M4 3l3 4h-2v10h2l-3 4l-3-4h2v-10h-2Z",
                rgb(0x2f, 0x6f, 0xba),
            ),
        ],
    );
    let ink = rgb(0x26, 0x26, 0x26);
    let white = rgb(255, 255, 255);
    // Text Fill: a solid A over a colour bar.
    icons.add_solid(
        "shape.textFill",
        &[
            (
                "M5 15l5-13h2l5 13h-2.4l-1.3-3.5h-4.6l-1.3 3.5ZM9.5 9.6h3.2l-1.6-4.5Z",
                ink,
            ),
            ("M3 18h16v4h-16Z", rgb(0x44, 0x72, 0xc4)),
        ],
    );
    // Text Outline: a hollow A over a colour bar.
    icons.add_solid(
        "shape.textOutline",
        &[
            (
                "M5 15l5-13h2l5 13h-2.4l-1.3-3.5h-4.6l-1.3 3.5ZM9.5 9.6h3.2l-1.6-4.5Z",
                white,
            ),
            (
                "M5 15l5-13h2l5 13h-2.4l-1.3-3.5h-4.6l-1.3 3.5ZM9.5 9.6h3.2l-1.6-4.5Z",
                None,
            ),
            ("M3 18h16v4h-16Z", rgb(0x26, 0x26, 0x26)),
        ],
    );
    // Text Effects: a blue A with a soft ring.
    icons.add_solid(
        "shape.textEffects",
        &[
            (
                "M2 12a9 9 0 1 0 18 0a9 9 0 1 0-18 0Z",
                rgb(0xd6, 0xe4, 0xf5),
            ),
            (
                "M6 18l5-13h2l5 13h-2.4l-1.3-3.5h-4.6l-1.3 3.5ZM10.5 12.6h3.2l-1.6-4.5Z",
                rgb(0x2f, 0x6f, 0xba),
            ),
        ],
    );
}
