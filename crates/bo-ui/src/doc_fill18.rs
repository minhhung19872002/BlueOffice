//! **Fill Effects** of Design › Page Color (BlueDoc layout round 8, docs/ui/REPORT.md "Dàn
//! trang vòng 8"): the page background as a gradient (one colour with a dark–light amount,
//! two colours, or a preset; six shading styles with their variants), a texture (BlueOffice's
//! own procedural materials, or another picture tiled), a two-colour pattern, or a picture
//! stretched over the page. OK sets the page fill (`setPageFill`: the VML page background,
//! ECMA-376 Part 4 §19.1.2.5). A child module of `doc_forms`.
use super::{t, DocForm, DocFormKind, FormContext, FormOut, Kind};
use crate::strings::Strings;
use crate::{Font, Painter};
use bo_render::Rgba;
use serde_json::{json, Value};

/// Whether `kind` is this module's dialog.
pub(super) fn handles(kind: DocFormKind) -> bool {
    kind == DocFormKind::FillEffects
}

/// Dialog size (reference pixels at 125 %).
const SIZE: (f32, f32) = (612.0, 560.0);
/// Sample box, on every tab.
const SAMPLE: [f32; 4] = [452.0, 380.0, 140.0, 110.0];
/// Variants of the Gradient tab (2 × 2 thumbnails).
const VARIANTS: [f32; 4] = [216.0, 300.0, 210.0, 190.0];
/// Texture grid (5 × 5).
const TEXTURES: [f32; 4] = [24.0, 96.0, 400.0, 330.0];
/// Pattern grid (8 × 6).
const PATTERN_GRID: [f32; 4] = [24.0, 96.0, 400.0, 250.0];
/// Picture box.
const PICTURE: [f32; 4] = [24.0, 96.0, 400.0, 300.0];

/// Shading styles: (radio id, key, English).
const STYLES: [(&str, &str, &str); 6] = [
    ("stH", "doc18.horizontal", "Horizontal"),
    ("stV", "doc18.vertical", "Vertical"),
    ("stDU", "doc18.diagonalUp", "Diagonal up"),
    ("stDD", "doc18.diagonalDown", "Diagonal down"),
    ("stC", "doc18.fromCorner", "From corner"),
    ("stM", "doc18.fromCenter", "From center"),
];

/// BlueOffice's own preset gradients: (key, English, colours top to bottom).
const PRESETS: [(&str, &str, &[&str]); 8] = [
    ("doc18.pDawn", "Dawn", &["FDE2C2", "F7A8B8", "8FA7E0"]),
    ("doc18.pSea", "Sea", &["E3F4FB", "6CC3E0", "1D5D8C"]),
    ("doc18.pMeadow", "Meadow", &["F4FBE3", "A7D676", "3D7A2B"]),
    ("doc18.pSand", "Sand", &["FFF7E6", "E8CFA0", "B08A52"]),
    ("doc18.pDusk", "Dusk", &["2B1E4A", "8A3C78", "F08A5D"]),
    ("doc18.pSteel", "Steel", &["F2F4F7", "AEB6C2", "5A6472"]),
    ("doc18.pRose", "Rose", &["FFF0F3", "F4A3B8", "B23A62"]),
    ("doc18.pLagoon", "Lagoon", &["E0FFF8", "41C9B0", "0E4D64"]),
];

/// Dialog state.
pub(crate) struct Fill18 {
    /// Shading style (index into [`STYLES`]) and variant (0..4).
    style: usize,
    variant: usize,
    /// Texture chosen (index into the texture library) or `None` for another picture.
    texture: Option<usize>,
    /// Another texture picked (name, base64).
    other: Option<(String, String)>,
    /// Pattern chosen (index into the slide pattern presets).
    pattern: usize,
    /// Picture picked (name, base64).
    picture: Option<(String, String)>,
    /// The host's next picked file is for the Texture tab (else the Picture tab).
    pick_texture: bool,
    /// A picture was asked of the host and has not come yet.
    awaiting: bool,
}

/// A group box on the Gradient tab.
fn group(f: &mut DocForm, id: &'static str, r: [f32; 4], text: &str) {
    f.form.add(id, Some(0), r, Kind::Group(text.to_owned()));
}

fn hex_rgb(h: &str) -> (u8, u8, u8) {
    super::r15::parse_hex(h).unwrap_or((255, 255, 255))
}

fn rgba(c: (u8, u8, u8)) -> Rgba {
    Rgba(c.0, c.1, c.2, 255)
}

fn mix(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let m = |x: u8, y: u8| {
        (f32::from(x) + (f32::from(y) - f32::from(x)) * t.clamp(0.0, 1.0)).round() as u8
    };
    (m(a.0, b.0), m(a.1, b.1), m(a.2, b.2))
}

/// Open Fill Effects with the page fill in `c.extra["r18"]`.
pub(super) fn open(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::FillEffects,
        &tr("doc18.title", "Fill Effects"),
        SIZE,
        vec![
            tr("doc18.gradient", "Gradient"),
            tr("doc18.texture", "Texture"),
            tr("doc18.pattern", "Pattern"),
            tr("doc18.picture", "Picture"),
        ],
    );
    let x = &c.extra["r18"];
    let page = x["color"].as_str().unwrap_or("FFFFFF").to_owned();
    let fill = &x["fill"];
    let color2 = fill["color2"].as_object().map_or_else(
        || "FFFFFF".into(),
        |o| {
            let n = |k: &str| o.get(k).and_then(Value::as_u64).unwrap_or(255).min(255) as u8;
            super::r15::to_hex((n("r"), n("g"), n("b")))
        },
    );
    let palette: Vec<String> = c.extra["palette"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    f.form.palette_labels = vec![
        tr("doc12.automatic", "Automatic"),
        tr("doc12.themeColors", "Theme Colors"),
        tr("doc12.standardColors", "Standard Colors"),
        tr("doc12.moreColors", "More Colors..."),
    ];
    let color_pick = |f: &mut DocForm, page: usize, id: &'static str, r: [f32; 4], hex: &str| {
        f.form.add(
            id,
            Some(page),
            r,
            Kind::ColorPick {
                hex: hex.to_owned(),
                palette: palette.clone(),
            },
        );
    };
    // Gradient tab.
    let kind = fill["kind"].as_str().unwrap_or("");
    let gradient = matches!(kind, "gradient" | "gradientRadial");
    let preset = gradient && fill["stops"].as_array().is_some_and(|a| a.len() > 2);
    let one = gradient && fill["color2Expr"].is_string();
    group(
        &mut f,
        "colorsG",
        [16.0, 84.0, 580.0, 150.0],
        &tr("doc18.colors", "Colors"),
    );
    f.radio_on(
        0,
        "one",
        "colors",
        [32.0, 108.0, 160.0, 24.0],
        &tr("doc18.oneColor", "One color"),
        one,
    );
    f.radio_on(
        0,
        "two",
        "colors",
        [32.0, 138.0, 160.0, 24.0],
        &tr("doc18.twoColors", "Two colors"),
        !one && !preset,
    );
    f.radio_on(
        0,
        "preset",
        "colors",
        [32.0, 168.0, 160.0, 24.0],
        &tr("doc18.preset", "Preset"),
        preset,
    );
    f.label_on(
        0,
        "c1L",
        [216.0, 100.0, 120.0, 18.0],
        &tr("doc18.color1", "Color 1:"),
    );
    color_pick(&mut f, 0, "c1", [216.0, 120.0, 180.0, 26.0], &page);
    f.label_on(
        0,
        "c2L",
        [216.0, 156.0, 120.0, 18.0],
        &tr("doc18.color2", "Color 2:"),
    );
    color_pick(&mut f, 0, "c2", [216.0, 176.0, 180.0, 26.0], &color2);
    f.label_on(
        0,
        "darkL",
        [420.0, 100.0, 160.0, 18.0],
        &tr("doc18.darkLight", "Dark ↔ Light:"),
    );
    f.form.add(
        "dark",
        Some(0),
        [420.0, 120.0, 90.0, 26.0],
        Kind::Spin {
            value: 50,
            min: 0,
            max: 100,
        },
    );
    f.label_on(
        0,
        "presetL",
        [420.0, 156.0, 160.0, 18.0],
        &tr("doc18.presetColors", "Preset colors:"),
    );
    f.form.add(
        "presets",
        Some(0),
        [420.0, 176.0, 160.0, 26.0],
        Kind::Combo {
            items: PRESETS.iter().map(|p| tr(p.0, p.1)).collect(),
            sel: 0,
        },
    );
    group(
        &mut f,
        "stylesG",
        [16.0, 250.0, 184.0, 240.0],
        &tr("doc18.shadingStyles", "Shading styles"),
    );
    let angle = fill["angle"].as_i64().unwrap_or(0);
    let style = match (kind, angle) {
        ("gradientRadial", _) => {
            let fp = fill["focusPosition"].as_array();
            let centre = fp.is_some_and(|a| a.iter().all(|v| v.as_i64() == Some(500)));
            if centre {
                5
            } else {
                4
            }
        }
        (_, -90 | 90) => 1,
        (_, -135 | 45) => 2,
        (_, -45 | 135) => 3,
        _ => 0,
    };
    for (i, (id, key, en)) in STYLES.iter().enumerate() {
        f.radio_on(
            0,
            id,
            "style",
            [32.0, 276.0 + 34.0 * i as f32, 160.0, 24.0],
            &tr(key, en),
            i == style,
        );
    }
    group(
        &mut f,
        "varG",
        [208.0, 250.0, 226.0, 240.0],
        &tr("doc18.variants", "Variants"),
    );
    f.form.add("variants", Some(0), VARIANTS, Kind::Canvas);
    // Texture tab.
    f.label_on(
        1,
        "texL",
        [24.0, 76.0, 120.0, 18.0],
        &tr("doc18.textureL", "Texture:"),
    );
    f.form.add("textures", Some(1), TEXTURES, Kind::Canvas);
    f.label_on(1, "texName", [24.0, 434.0, 400.0, 18.0], "");
    f.button_on(
        1,
        "otherTexture",
        [24.0, 460.0, 170.0, 30.0],
        &tr("doc18.otherTexture", "Other Texture..."),
    );
    // Pattern tab.
    f.label_on(
        2,
        "patL",
        [24.0, 76.0, 120.0, 18.0],
        &tr("doc18.patternL", "Pattern:"),
    );
    f.form.add("patterns", Some(2), PATTERN_GRID, Kind::Canvas);
    f.label_on(2, "patName", [24.0, 352.0, 400.0, 18.0], "");
    f.label_on(
        2,
        "fgL",
        [24.0, 384.0, 160.0, 18.0],
        &tr("doc18.foreground", "Foreground:"),
    );
    color_pick(
        &mut f,
        2,
        "fg",
        [24.0, 404.0, 180.0, 26.0],
        if kind == "pattern" { &page } else { "000000" },
    );
    f.label_on(
        2,
        "bgL",
        [232.0, 384.0, 160.0, 18.0],
        &tr("doc18.background", "Background:"),
    );
    color_pick(
        &mut f,
        2,
        "bg",
        [232.0, 404.0, 180.0, 26.0],
        if kind == "pattern" { &color2 } else { "FFFFFF" },
    );
    // Picture tab.
    f.form.add("picture", Some(3), PICTURE, Kind::Canvas);
    f.button_on(
        3,
        "selectPicture",
        [24.0, 408.0, 170.0, 30.0],
        &tr("doc18.selectPicture", "Select Picture..."),
    );
    f.check_on(
        3,
        "lockAspect",
        [24.0, 448.0, 260.0, 24.0],
        &tr("doc18.lockAspect", "Lock picture aspect ratio"),
        fill["lockAspect"].as_bool().unwrap_or(false),
    );
    // Every tab.
    f.label(
        "sampleL",
        [452.0, 356.0, 120.0, 18.0],
        &tr("doc18.sample", "Sample:"),
    );
    f.form.add("sample", None, SAMPLE, Kind::Canvas);
    f.button(
        "ok",
        [372.0, 510.0, 108.0, 32.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [490.0, 510.0, 108.0, 32.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f.words = PRESETS
        .iter()
        .map(|p| (p.1.to_owned(), tr(p.0, p.1)))
        .collect();
    f.words.extend(
        crate::slide_textures22::NAMES
            .iter()
            .map(|n| ((*n).to_owned(), (*n).to_owned())),
    );
    let variant = match (style, fill["focus"].as_i64().unwrap_or(100)) {
        (4, _) => {
            let fp = fill["focusPosition"].as_array();
            let at = |i: usize| {
                fp.and_then(|a| a.get(i))
                    .and_then(Value::as_i64)
                    .unwrap_or(0)
            };
            usize::from(at(0) > 500) + 2 * usize::from(at(1) > 500)
        }
        (_, 0) => 1,
        (_, 50) => 2,
        (_, -50) => 3,
        _ => 0,
    };
    f.r18 = Some(Box::new(Fill18 {
        style,
        variant,
        texture: Some(0),
        other: None,
        pattern: 0,
        picture: None,
        pick_texture: false,
        awaiting: false,
    }));
    f.form.tab = match kind {
        "tile" => 1,
        "pattern" => 2,
        "frame" => 3,
        _ => 0,
    };
    f.r18_enable();
    f
}

/// Colour stops of the dialog's gradient, from the start of the shading to its end.
fn stops_of(f: &DocForm) -> Vec<(f32, (u8, u8, u8))> {
    let c1 = hex_rgb(&f.form.color("c1"));
    match f.chosen("colors") {
        Some("one") => {
            let k = f.form.num("dark").clamp(0, 100) as f32 / 100.0;
            vec![(0.0, c1), (1.0, one_color2(c1, k))]
        }
        Some("preset") => {
            let p = PRESETS[f.sel("presets").unwrap_or(0).min(PRESETS.len() - 1)].2;
            let n = (p.len() - 1).max(1) as f32;
            p.iter()
                .enumerate()
                .map(|(i, h)| (i as f32 / n, hex_rgb(h)))
                .collect()
        }
        _ => vec![(0.0, c1), (1.0, hex_rgb(&f.form.color("c2")))],
    }
}

/// The second colour of a one-colour gradient at dark–light `k` (0 = black, 1 = white).
fn one_color2(c: (u8, u8, u8), k: f32) -> (u8, u8, u8) {
    if k < 0.5 {
        mix((0, 0, 0), c, k * 2.0)
    } else {
        mix(c, (255, 255, 255), (k - 0.5) * 2.0)
    }
}

/// The `fill darken(n)` / `fill lighten(n)` formula of a one-colour gradient (Part 4,
/// §19.1.2.5 color2) at dark–light `k`.
fn one_color_expr(k: f32) -> String {
    if k < 0.5 {
        format!("fill darken({})", (k * 2.0 * 255.0).round() as i32)
    } else {
        format!(
            "fill lighten({})",
            ((1.0 - (k - 0.5) * 2.0) * 255.0).round() as i32
        )
    }
}

/// VML angle, focus, radial focus position of shading style `style`, variant `v`.
fn shading(style: usize, v: usize) -> (bool, i32, i32, (i32, i32)) {
    let focus = [100, 0, 50, -50][v.min(3)];
    match style {
        1 => (false, -90, focus, (0, 0)),
        2 => (false, -135, focus, (0, 0)),
        3 => (false, -45, focus, (0, 0)),
        4 => {
            let p = [(0, 0), (1000, 0), (0, 1000), (1000, 1000)][v.min(3)];
            (true, 0, 100, p)
        }
        5 => (true, 0, if v == 1 { 0 } else { 100 }, (500, 500)),
        _ => (false, 0, focus, (0, 0)),
    }
}

/// An 8 × 8 1-bit BMP of `rows` (bit 7 = left; set = the pattern colour, palette index 1
/// black so that readers take it as the foreground).
pub(crate) fn pattern_bmp(rows: [u8; 8]) -> Vec<u8> {
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
    b.extend_from_slice(&[255, 255, 255, 0, 0, 0, 0, 0]);
    for r in rows.iter().rev() {
        b.extend_from_slice(&[*r, 0, 0, 0]);
    }
    b
}

fn b64(bytes: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

impl DocForm {
    fn fill18(&self) -> Option<&Fill18> {
        self.r18.as_deref()
    }

    /// Controls that only apply to the chosen colours.
    fn r18_enable(&mut self) {
        let c = self.chosen("colors").unwrap_or("two");
        self.form.enable("c1", c != "preset");
        self.form.enable("c2", c == "two");
        self.form.enable("dark", c == "one");
        self.form.enable("presets", c == "preset");
        let tex = self.fill18().and_then(|s| s.texture);
        let name = tex
            .and_then(|i| crate::slide_textures22::NAMES.get(i))
            .map(|n| (*n).to_owned())
            .or_else(|| {
                self.fill18()
                    .and_then(|s| s.other.as_ref().map(|o| o.0.clone()))
            })
            .unwrap_or_default();
        self.set_label("texName", &name);
        let pat = self.fill18().map_or(0, |s| s.pattern);
        let pname = crate::slide_bgpane::PATTERNS
            .get(pat)
            .copied()
            .unwrap_or("");
        self.set_label("patName", pname);
    }

    /// A click on a Fill Effects canvas at (x, y) reference pixels inside it.
    pub(super) fn r18_canvas(&mut self, id: &str, x: f32, y: f32) -> bool {
        if !handles(self.kind) {
            return false;
        }
        let Some(st) = self.r18.as_mut() else {
            return false;
        };
        match id {
            "variants" => {
                let col = usize::from(x > VARIANTS[2] / 2.0);
                let row = usize::from(y > VARIANTS[3] / 2.0);
                st.variant = row * 2 + col;
            }
            "textures" => {
                let (cw, ch) = (TEXTURES[2] / 5.0, TEXTURES[3] / 5.0);
                let i = (y / ch) as usize * 5 + (x / cw) as usize;
                if i < crate::slide_textures22::NAMES.len() {
                    st.texture = Some(i);
                    st.other = None;
                }
            }
            "patterns" => {
                let (cw, ch) = (PATTERN_GRID[2] / 8.0, PATTERN_GRID[3] / 6.0);
                let i = (y / ch) as usize * 8 + (x / cw) as usize;
                if i < crate::slide_bgpane::PATTERNS.len() {
                    st.pattern = i;
                }
            }
            _ => {
                if self.radios.iter().any(|r| r.id == id && r.enabled) {
                    self.set_radio(id);
                    if let Some(i) = STYLES.iter().position(|s| s.0 == id) {
                        if let Some(st) = self.r18.as_mut() {
                            st.style = i;
                            st.variant = 0;
                        }
                    }
                    self.r18_enable();
                    return true;
                }
                return false;
            }
        }
        self.r18_enable();
        true
    }

    /// A control of Fill Effects changed.
    pub(super) fn r18_changed(&mut self, _id: &str) {
        self.r18_enable();
    }

    /// Buttons of Fill Effects: Other Texture… / Select Picture… ask the host for a picture.
    pub(super) fn r18_clicked(&mut self, id: &str) -> Option<FormOut> {
        if !handles(self.kind) {
            return None;
        }
        let st = self.r18.as_mut()?;
        match id {
            "otherTexture" | "selectPicture" => {
                st.pick_texture = id == "otherTexture";
                st.awaiting = true;
                Some(FormOut::Run(
                    vec![json!({ "cmd": "ui.action", "id": "r18.pickPicture" })],
                    false,
                ))
            }
            _ => None,
        }
    }

    /// The tab shown (tests).
    #[cfg(test)]
    pub(crate) fn form_tab_for_test(&self) -> usize {
        self.form.tab
    }

    /// The dialog waits for a picture from the host (Other Texture… / Select Picture…).
    pub(crate) fn r18_awaiting(&self) -> bool {
        self.fill18().is_some_and(|s| s.awaiting)
    }

    /// A picture the host read for Other Texture… / Select Picture….
    pub(crate) fn r18_picture(&mut self, name: &str, base64: &str) -> bool {
        let Some(st) = self.r18.as_mut() else {
            return false;
        };
        st.awaiting = false;
        let pick = Some((name.to_owned(), base64.to_owned()));
        if st.pick_texture {
            st.other = pick;
            st.texture = None;
        } else {
            st.picture = pick;
        }
        self.r18_enable();
        true
    }

    /// The `setPageFill` command of the tab shown.
    pub(crate) fn r18_command(&self) -> Option<Value> {
        let st = self.fill18()?;
        Some(match self.form.tab {
            1 => {
                let (title, data) = match (&st.other, st.texture) {
                    (Some((n, d)), _) => (n.clone(), d.clone()),
                    (None, Some(i)) => (
                        crate::slide_textures22::NAMES.get(i)?.to_string(),
                        b64(&crate::slide_textures22::png(i)?),
                    ),
                    _ => return None,
                };
                json!({ "cmd": "setPageFill", "color": "FFFFFF",
                        "fill": { "kind": "tile", "title": title }, "picture": data })
            }
            2 => {
                let name = crate::slide_bgpane::PATTERNS.get(st.pattern)?;
                let bits = crate::slide_bgpane::pattern_bits(name);
                let (r, g, b) = hex_rgb(&self.form.color("bg"));
                let fg = self.form.color("fg");
                json!({ "cmd": "setPageFill",
                        "color": if fg.is_empty() { "000000".to_owned() } else { fg },
                        "fill": { "kind": "pattern", "title": name,
                                  "color2": { "r": r, "g": g, "b": b } },
                        "picture": b64(&pattern_bmp(bits)) })
            }
            3 => {
                let (name, data) = st.picture.clone()?;
                json!({ "cmd": "setPageFill", "color": "FFFFFF",
                        "fill": { "kind": "frame", "title": name, "lockAspect": self.on("lockAspect") },
                        "picture": data })
            }
            _ => {
                let (radial, angle, focus, pos) = shading(st.style, st.variant);
                let stops = stops_of(self);
                let c1 = stops.first().map_or((255, 255, 255), |s| s.1);
                let mut fill = json!({
                    "kind": if radial { "gradientRadial" } else { "gradient" },
                    "angle": angle, "focus": focus, "focusPosition": [pos.0, pos.1],
                });
                match self.chosen("colors") {
                    Some("preset") => {
                        fill["stops"] = Value::Array(
                            stops
                                .iter()
                                .map(|(t, c)| {
                                    json!([(t * 1000.0).round() as u32, { "r": c.0, "g": c.1, "b": c.2 }])
                                })
                                .collect(),
                        );
                        fill["method"] = json!("none");
                        let p = PRESETS[self.sel("presets").unwrap_or(0).min(PRESETS.len() - 1)];
                        fill["title"] = json!(p.1);
                    }
                    Some("one") => {
                        let k = self.form.num("dark").clamp(0, 100) as f32 / 100.0;
                        let c2 = one_color2(c1, k);
                        fill["color2"] = json!({ "r": c2.0, "g": c2.1, "b": c2.2 });
                        fill["color2Expr"] = json!(one_color_expr(k));
                        fill["method"] = json!("linear sigma");
                    }
                    _ => {
                        let c2 = stops.last().map_or((255, 255, 255), |s| s.1);
                        fill["color2"] = json!({ "r": c2.0, "g": c2.1, "b": c2.2 });
                    }
                }
                json!({ "cmd": "setPageFill", "color": super::r15::to_hex(c1), "fill": fill })
            }
        })
    }

    /// OK of Fill Effects.
    pub(super) fn r18_ok(&mut self) -> Option<FormOut> {
        if !handles(self.kind) {
            return None;
        }
        Some(match self.r18_command() {
            Some(cmd) => FormOut::Run(vec![cmd], true),
            None => FormOut::Close,
        })
    }
}

/// Colour at `t` of `stops` with the dialog's `focus` mirroring.
fn shade_at(stops: &[(f32, (u8, u8, u8))], focus: i32, t: f32) -> (u8, u8, u8) {
    let g = (focus.unsigned_abs() as f32 / 100.0).clamp(0.0, 1.0);
    let mut u = if g >= 0.999 {
        t
    } else if g <= 0.001 {
        1.0 - t
    } else if t <= g {
        t / g
    } else {
        (1.0 - t) / (1.0 - g)
    };
    if focus < 0 {
        u = 1.0 - u;
    }
    let first = stops.first().map_or((255, 255, 255), |s| s.1);
    for w in stops.windows(2) {
        let ((t0, a), (t1, b)) = (w[0], w[1]);
        if u <= t1 {
            return mix(a, b, if t1 > t0 { (u - t0) / (t1 - t0) } else { 1.0 });
        }
    }
    stops.last().map_or(first, |s| s.1)
}

/// Paint `r` with the gradient of shading style `style`, variant `v`, in 1 px steps.
fn paint_gradient(
    p: &mut dyn Painter,
    r: [f32; 4],
    stops: &[(f32, (u8, u8, u8))],
    style: usize,
    v: usize,
) {
    let (radial, angle, focus, pos) = shading(style, v);
    let (w, h) = (r[2].max(1.0), r[3].max(1.0));
    if radial {
        let (fx, fy) = (pos.0 as f32 / 1000.0 * w, pos.1 as f32 / 1000.0 * h);
        let (ex, ey) = (fx.max(w - fx), fy.max(h - fy));
        let n = 48;
        for i in (1..=n).rev() {
            let t = i as f32 / n as f32;
            let c = shade_at(stops, focus, t);
            let (hw, hh) = (ex * t, ey * t);
            let x0 = (fx - hw).max(0.0);
            let y0 = (fy - hh).max(0.0);
            let x1 = (fx + hw).min(w);
            let y1 = (fy + hh).min(h);
            p.fill_rect(r[0] + x0, r[1] + y0, x1 - x0, y1 - y0, rgba(c));
        }
        return;
    }
    let a = (angle as f32).to_radians();
    let (dx, dy) = (-a.sin(), a.cos());
    let half = f32::midpoint(w * dx.abs(), h * dy.abs());
    let step = 2.0;
    let mut y = 0.0;
    while y < h {
        let mut x = 0.0;
        while x < w {
            let proj = (x + step / 2.0 - w / 2.0) * dx + (y + step / 2.0 - h / 2.0) * dy;
            let t = ((proj + half) / (2.0 * half)).clamp(0.0, 1.0);
            p.fill_rect(
                r[0] + x,
                r[1] + y,
                step.min(w - x),
                step.min(h - y),
                rgba(shade_at(stops, focus, t)),
            );
            x += step;
        }
        y += step;
    }
}

fn frame(p: &mut dyn Painter, r: [f32; 4], c: Rgba) {
    p.fill_rect(r[0], r[1], r[2], 1.0, c);
    p.fill_rect(r[0], r[1] + r[3] - 1.0, r[2], 1.0, c);
    p.fill_rect(r[0], r[1], 1.0, r[3], c);
    p.fill_rect(r[0] + r[2] - 1.0, r[1], 1.0, r[3], c);
}

fn paint_texture(p: &mut dyn Painter, r: [f32; 4], i: usize) {
    let n = 24usize;
    let px = crate::slide_textures22::preview(i, n);
    let (cw, ch) = (r[2] / n as f32, r[3] / n as f32);
    for (k, c) in px.iter().enumerate() {
        let (x, y) = ((k % n) as f32, (k / n) as f32);
        p.fill_rect(r[0] + x * cw, r[1] + y * ch, cw.ceil(), ch.ceil(), *c);
    }
}

fn paint_pattern(p: &mut dyn Painter, r: [f32; 4], bits: [u8; 8], fg: Rgba, bg: Rgba, cell: f32) {
    p.fill_rect(r[0], r[1], r[2], r[3], bg);
    let mut y = 0.0;
    let mut row = 0usize;
    while y < r[3] {
        let mut x = 0.0;
        let mut col = 0usize;
        while x < r[2] {
            if bits[row % 8] & (0x80 >> (col % 8)) != 0 {
                p.fill_rect(
                    r[0] + x,
                    r[1] + y,
                    cell.min(r[2] - x),
                    cell.min(r[3] - y),
                    fg,
                );
            }
            x += cell;
            col += 1;
        }
        y += cell;
        row += 1;
    }
}

/// Paint a Fill Effects canvas; `false` when `id` is not one.
pub(super) fn paint_canvas(
    f: &DocForm,
    id: &str,
    r: [f32; 4],
    p: &mut dyn Painter,
    font: &Font<'_>,
) -> bool {
    if !handles(f.kind) {
        return false;
    }
    let Some(st) = f.fill18() else {
        return false;
    };
    let edge = Rgba(0x7a, 0x7a, 0x7a, 255);
    let hot = Rgba(0x00, 0x5f, 0xb8, 255);
    match id {
        "variants" => {
            let stops = stops_of(f);
            let (cw, ch) = (r[2] / 2.0, r[3] / 2.0);
            for v in 0..4 {
                let (x, y) = (r[0] + (v % 2) as f32 * cw, r[1] + (v / 2) as f32 * ch);
                let b = [x + 8.0, y + 8.0, cw - 16.0, ch - 16.0];
                paint_gradient(p, b, &stops, st.style, v);
                frame(p, [b[0] - 1.0, b[1] - 1.0, b[2] + 2.0, b[3] + 2.0], edge);
                if v == st.variant {
                    frame(p, [b[0] - 4.0, b[1] - 4.0, b[2] + 8.0, b[3] + 8.0], hot);
                }
            }
        }
        "textures" => {
            let (cw, ch) = (r[2] / 5.0, r[3] / 5.0);
            for i in 0..crate::slide_textures22::NAMES.len().min(25) {
                let (x, y) = (r[0] + (i % 5) as f32 * cw, r[1] + (i / 5) as f32 * ch);
                let b = [x + 4.0, y + 4.0, cw - 8.0, ch - 8.0];
                paint_texture(p, b, i);
                frame(p, b, edge);
                if st.texture == Some(i) {
                    frame(p, [b[0] - 3.0, b[1] - 3.0, b[2] + 6.0, b[3] + 6.0], hot);
                }
            }
        }
        "patterns" => {
            let (fg, bg) = (
                rgba(hex_rgb(&f.form.color("fg"))),
                rgba(hex_rgb(&f.form.color("bg"))),
            );
            let (cw, ch) = (r[2] / 8.0, r[3] / 6.0);
            let k = (r[2] / PATTERN_GRID[2]).max(0.5);
            for (i, name) in crate::slide_bgpane::PATTERNS.iter().enumerate() {
                let (x, y) = (r[0] + (i % 8) as f32 * cw, r[1] + (i / 8) as f32 * ch);
                let b = [x + 4.0, y + 4.0, cw - 8.0, ch - 8.0];
                paint_pattern(
                    p,
                    b,
                    crate::slide_bgpane::pattern_bits(name),
                    fg,
                    bg,
                    (1.25 * k).round().max(1.0),
                );
                frame(p, b, edge);
                if st.pattern == i {
                    frame(p, [b[0] - 3.0, b[1] - 3.0, b[2] + 6.0, b[3] + 6.0], hot);
                }
            }
        }
        "picture" => {
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(255, 255, 255, 255));
            frame(p, r, edge);
            if let Some((name, _)) = &st.picture {
                p.text_font(
                    name,
                    r[0] + 12.0,
                    r[1] + r[3] / 2.0 - font.size,
                    font,
                    Rgba(0, 0, 0, 255),
                );
            }
        }
        "sample" => {
            p.fill_rect(r[0], r[1], r[2], r[3], Rgba(255, 255, 255, 255));
            match f.form.tab {
                1 => {
                    if let Some(i) = st.texture {
                        paint_texture(p, r, i);
                    }
                }
                2 => {
                    let name = crate::slide_bgpane::PATTERNS
                        .get(st.pattern)
                        .copied()
                        .unwrap_or("pct5");
                    let (fg, bg) = (
                        rgba(hex_rgb(&f.form.color("fg"))),
                        rgba(hex_rgb(&f.form.color("bg"))),
                    );
                    paint_pattern(p, r, crate::slide_bgpane::pattern_bits(name), fg, bg, 1.0);
                }
                3 => {}
                _ => paint_gradient(p, r, &stops_of(f), st.style, st.variant),
            }
            frame(p, r, edge);
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(extra: Value) -> DocForm {
        let s = Strings::load("en-US");
        let c = FormContext {
            extra,
            ..FormContext::default()
        };
        open(&s, &c)
    }

    #[test]
    fn gradient_tab_builds_the_vml_fill() {
        let mut f = form(json!({ "r18": { "color": "2070C0" } }));
        assert_eq!(f.form.tab, 0);
        f.form.set_color("c2", "FFE060");
        // Vertical, variant 3 (the second colour in the middle).
        assert!(f.r18_canvas("stV", 0.0, 0.0));
        assert!(f.r18_canvas("variants", VARIANTS[2] * 0.25, VARIANTS[3] * 0.75));
        let c = f.r18_command().unwrap();
        assert_eq!(c["cmd"], "setPageFill");
        assert_eq!(c["color"], "2070C0");
        assert_eq!(c["fill"]["kind"], "gradient");
        assert_eq!(c["fill"]["angle"], -90);
        assert_eq!(c["fill"]["focus"], 50);
        assert_eq!(c["fill"]["color2"]["g"], 0xE0);
        // From center: a rectangular gradient around the middle.
        f.r18_canvas("stM", 0.0, 0.0);
        let c = f.r18_command().unwrap();
        assert_eq!(c["fill"]["kind"], "gradientRadial");
        assert_eq!(c["fill"]["focusPosition"], json!([500, 500]));
        // One colour: the second colour is a darken / lighten formula of the first.
        f.r18_canvas("one", 0.0, 0.0);
        f.form.set_num("dark", 25);
        let c = f.r18_command().unwrap();
        assert_eq!(c["fill"]["color2Expr"], "fill darken(128)");
    }

    #[test]
    fn texture_pattern_and_picture_tabs() {
        use base64::Engine as _;
        let mut f = form(json!({}));
        f.form.tab = 1;
        let (cw, ch) = (TEXTURES[2] / 5.0, TEXTURES[3] / 5.0);
        assert!(f.r18_canvas("textures", cw * 2.5, ch * 1.5));
        let c = f.r18_command().unwrap();
        assert_eq!(c["fill"]["kind"], "tile");
        assert_eq!(c["fill"]["title"], crate::slide_textures22::NAMES[7]);
        assert!(c["picture"].as_str().is_some_and(|s| s.len() > 100));
        f.form.tab = 2;
        f.r18_canvas("patterns", PATTERN_GRID[2] / 8.0 * 1.5, 1.0);
        f.form.set_color("fg", "C04020");
        let c = f.r18_command().unwrap();
        assert_eq!(c["fill"]["kind"], "pattern");
        assert_eq!(c["color"], "C04020");
        assert_eq!(c["fill"]["title"], crate::slide_bgpane::PATTERNS[1]);
        // The bitmap reads back as the pattern's bits.
        let bmp = base64::engine::general_purpose::STANDARD
            .decode(c["picture"].as_str().unwrap())
            .unwrap();
        assert_eq!(
            bo_layout::pattern_tile(&bmp),
            Some(crate::slide_bgpane::pattern_bits(
                crate::slide_bgpane::PATTERNS[1]
            ))
        );
        // Picture: nothing until one is picked.
        f.form.tab = 3;
        assert!(f.r18_command().is_none());
        assert_eq!(
            f.r18_clicked("selectPicture"),
            Some(FormOut::Run(
                vec![json!({ "cmd": "ui.action", "id": "r18.pickPicture" })],
                false
            ))
        );
        assert!(f.r18_picture("photo.png", "iVBORw0KGgo="));
        let c = f.r18_command().unwrap();
        assert_eq!(c["fill"]["kind"], "frame");
        assert_eq!(c["picture"], "iVBORw0KGgo=");
    }

    #[test]
    fn opens_on_the_tab_of_the_current_fill() {
        let f = form(json!({ "r18": { "color": "FFFFFF",
            "fill": { "kind": "gradientRadial", "focusPosition": [1000, 1000], "focus": 100 } } }));
        assert_eq!(f.form.tab, 0);
        assert_eq!(f.chosen("style"), Some("stC"));
        assert_eq!(f.fill18().map(|s| s.variant), Some(3));
        let f = form(json!({ "r18": { "fill": { "kind": "pattern" } } }));
        assert_eq!(f.form.tab, 2);
    }
}
