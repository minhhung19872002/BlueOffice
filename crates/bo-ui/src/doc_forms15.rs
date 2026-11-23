//! BlueDoc dialogs of parity round 15 (docs/ui/REPORT.md "Vòng 105"), laid out from the
//! reference word processor's dialogs captured at 125 % (window less its 8 px invisible
//! border): **Colors** (More Colors… of every colour picker: a Standard tab with a hexagon of
//! 127 colours and a grey row, a Custom tab with a hue × saturation field, a luminance bar,
//! the RGB / HSL model and a hex box; New / Current swatch). A child module of `doc_forms`.
use super::{t, DocForm, DocFormKind, FormContext, FormOut, Kind};
use crate::strings::Strings;
use crate::{Font, Painter};
use bo_render::{PathSeg, Rgba};
use serde_json::json;

/// Whether `kind` is one of this module's dialogs.
pub(super) fn handles(kind: DocFormKind) -> bool {
    matches!(
        kind,
        DocFormKind::Colors | DocFormKind::ThemeColors | DocFormKind::ThemeFonts
    )
}

/// The hexagon of the Standard tab: 13 rows of 7…13…7 colours (sampled from the reference's
/// cells, black-box), top to bottom.
const HEX_ROWS: [&[&str]; 13] = [
    &[
        "003366", "336699", "3366CC", "003399", "000099", "0000CC", "000066",
    ],
    &[
        "006666", "006699", "0099CC", "0066CC", "0033CC", "0000FF", "3333FF", "333399",
    ],
    &[
        "008080", "009999", "33CCCC", "00CCFF", "0099FF", "0066FF", "3366FF", "3333CC", "666699",
    ],
    &[
        "339966", "00CC99", "00FFCC", "00FFFF", "33CCFF", "3399FF", "6699FF", "6666FF", "6600FF",
        "6600CC",
    ],
    &[
        "339933", "00CC66", "00FF99", "66FFCC", "66FFFF", "66CCFF", "99CCFF", "9999FF", "9966FF",
        "9933FF", "9900FF",
    ],
    &[
        "006600", "00CC00", "00FF00", "66FF99", "99FFCC", "CCFFFF", "CCECFF", "CCCCFF", "CC99FF",
        "CC66FF", "CC00FF", "9900CC",
    ],
    &[
        "003300", "008000", "33CC33", "66FF66", "99FF99", "CCFFCC", "FFFFFF", "FFCCFF", "FF99FF",
        "FF66FF", "FF00FF", "CC00CC", "660066",
    ],
    &[
        "336600", "009900", "66FF33", "99FF66", "CCFF99", "FFFFCC", "FFCCCC", "FF99CC", "FF66CC",
        "FF33CC", "CC0099", "800080",
    ],
    &[
        "333300", "669900", "99FF33", "CCFF66", "FFFF99", "FFCC99", "FF9999", "FF6699", "FF3399",
        "CC3399", "990099",
    ],
    &[
        "666633", "99CC00", "CCFF33", "FFFF66", "FFCC66", "FF9966", "FF7C80", "FF0066", "D60093",
        "993366",
    ],
    &[
        "808000", "CCCC00", "FFFF00", "FFCC00", "FF9933", "FF6600", "FF5050", "CC0066", "660033",
    ],
    &[
        "996633", "CC9900", "FF9900", "CC6600", "FF3300", "FF0000", "CC0000", "990033",
    ],
    &[
        "663300", "996600", "CC3300", "993300", "990000", "800000", "A50021",
    ],
];

/// Greys under the hexagon: upper and lower staggered rows.
const GREYS_TOP: [&str; 8] = [
    "F8F8F8", "DDDDDD", "B2B2B2", "808080", "5F5F5F", "333333", "1C1C1C", "080808",
];
const GREYS_BOTTOM: [&str; 7] = [
    "EAEAEA", "C0C0C0", "969696", "777777", "4D4D4D", "292929", "111111",
];

/// Canvas rectangles (reference pixels from the dialog's corner).
const HEX_CANVAS: [f32; 4] = [25.0, 104.0, 184.0, 214.0];
const SPECTRUM: [f32; 4] = [25.0, 108.0, 180.0, 140.0];
const LUM: [f32; 4] = [214.0, 108.0, 40.0, 142.0];
const SWATCH: [f32; 4] = [280.0, 296.0, 92.0, 120.0];

/// One cell of the Standard tab: centre (reference pixels), half width, half height, colour.
#[derive(Debug, Clone, Copy)]
struct Cell {
    cx: f32,
    cy: f32,
    rx: f32,
    ry: f32,
    hex: &'static str,
}

/// Every cell of the Standard tab (hexagon, the big white, the greys, the big black).
fn cells() -> Vec<Cell> {
    let mut out = Vec::with_capacity(144);
    for (r, row) in HEX_ROWS.iter().enumerate() {
        let k = (r as f32 - 6.0).abs();
        for (i, hex) in row.iter().enumerate() {
            out.push(Cell {
                cx: 26.0 + k * 7.0 + 7.0 + 14.0 * i as f32,
                cy: 190.0 + (r as f32 - 6.0) * 12.0,
                rx: 7.0,
                ry: 8.0,
                hex,
            });
        }
    }
    out.push(Cell {
        cx: 40.0,
        cy: 295.0,
        rx: 14.0,
        ry: 16.0,
        hex: "FFFFFF",
    });
    for (j, hex) in GREYS_TOP.iter().enumerate() {
        out.push(Cell {
            cx: 68.0 + 14.0 * j as f32,
            cy: 291.0,
            rx: 7.0,
            ry: 8.0,
            hex,
        });
    }
    for (j, hex) in GREYS_BOTTOM.iter().enumerate() {
        out.push(Cell {
            cx: 75.0 + 14.0 * j as f32,
            cy: 303.0,
            rx: 7.0,
            ry: 8.0,
            hex,
        });
    }
    out.push(Cell {
        cx: 194.0,
        cy: 295.0,
        rx: 14.0,
        ry: 16.0,
        hex: "000000",
    });
    out
}

/// State of the Colors dialog.
#[derive(Debug, Clone, Default)]
pub(crate) struct ColorsState {
    /// The colour the picker had (`RRGGBB`), shown under "Current".
    pub(crate) current: Option<String>,
    /// The colour picked here.
    pub(crate) new: Option<String>,
    /// Hue, saturation, luminance (0–1) of the Custom tab (kept apart so a grey keeps its
    /// hue while the luminance bar moves).
    pub(crate) hsl: (f32, f32, f32),
    /// What OK changes: `page` (Page Color) or `form` (the colour drop-down of the dialog
    /// kept under this one).
    pub(crate) target: String,
}

/// `RRGGBB` → (r, g, b).
pub(crate) fn parse_hex(h: &str) -> Option<(u8, u8, u8)> {
    let h = h.trim().trim_start_matches('#');
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some(((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

pub(crate) fn to_hex(c: (u8, u8, u8)) -> String {
    format!("{:02X}{:02X}{:02X}", c.0, c.1, c.2)
}

/// RGB (0–255) → HSL (0–1).
pub(crate) fn rgb_to_hsl(c: (u8, u8, u8)) -> (f32, f32, f32) {
    let (r, g, b) = (
        f32::from(c.0) / 255.0,
        f32::from(c.1) / 255.0,
        f32::from(c.2) / 255.0,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    let d = max - min;
    if d < 1e-6 {
        return (0.0, 0.0, l);
    }
    let s = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let h = if (max - r).abs() < 1e-6 {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if (max - g).abs() < 1e-6 {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h / 6.0, s, l)
}

/// HSL (0–1) → RGB (0–255).
pub(crate) fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let to = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    if s <= 0.0 {
        return (to(l), to(l), to(l));
    }
    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;
    let f = |mut t: f32| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    (to(f(h + 1.0 / 3.0)), to(f(h)), to(f(h - 1.0 / 3.0)))
}

/// Build round-15 dialog `kind`.
pub(super) fn open(kind: DocFormKind, s: &Strings, c: &FormContext) -> DocForm {
    match kind {
        DocFormKind::ThemeColors => open_theme_colors(s, c),
        DocFormKind::ThemeFonts => open_theme_fonts(s, c),
        _ => open_colors(s, c),
    }
}

/// Colors (394 × 423 measured): Standard / Custom tabs on a narrow page, OK / Cancel on the
/// right, New / Current swatch under them.
fn open_colors(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::Colors,
        &tr("doc15.colorsTitle", "Colors"),
        (394.0, 423.0),
        vec![
            tr("doc15.standard", "Standard"),
            tr("doc15.custom", "Custom"),
        ],
    );
    f.form.page_w = Some(244.0);
    f.form.page_h = Some(336.0);
    f.form.no_help = true;
    f.form.sizable = true;
    let v = &c.extra["r15"]["color"];
    let current = v["current"]
        .as_str()
        .filter(|h| parse_hex(h).is_some())
        .map(str::to_uppercase);
    let target = v["target"].as_str().unwrap_or("page").to_owned();
    let rgb = current
        .as_deref()
        .and_then(parse_hex)
        .unwrap_or((128, 128, 128));
    f.label_on(
        0,
        "colorsL",
        [24.0, 88.0, 100.0, 18.0],
        &tr("doc15.colorsL", "Colors:"),
    );
    f.form.add("hex6", Some(0), HEX_CANVAS, Kind::Canvas);
    f.label_on(
        1,
        "colorsL2",
        [24.0, 86.0, 100.0, 18.0],
        &tr("doc15.colorsL", "Colors:"),
    );
    f.form.add("spectrum", Some(1), SPECTRUM, Kind::Canvas);
    f.form.add("lum", Some(1), LUM, Kind::Canvas);
    f.label_on(
        1,
        "modelL",
        [24.0, 259.0, 90.0, 18.0],
        &tr("doc15.colorModel", "Color model:"),
    );
    f.form.add(
        "model",
        Some(1),
        [114.0, 255.0, 128.0, 26.0],
        Kind::Combo {
            items: vec!["RGB".into(), "HSL".into()],
            sel: 0,
        },
    );
    for (i, (id, sid, key, fb)) in [
        ("c1L", "c1", "doc15.red", "Red:"),
        ("c2L", "c2", "doc15.green", "Green:"),
        ("c3L", "c3", "doc15.blue", "Blue:"),
    ]
    .into_iter()
    .enumerate()
    {
        let y = 286.0 + 30.0 * i as f32;
        f.label_on(1, id, [24.0, y + 3.0, 90.0, 18.0], &tr(key, fb));
        let value = [rgb.0, rgb.1, rgb.2][i];
        f.form.add(
            sid,
            Some(1),
            [114.0, y, 65.0, 24.0],
            Kind::Spin {
                value: i32::from(value),
                min: 0,
                max: 255,
            },
        );
    }
    f.label_on(
        1,
        "hexL",
        [24.0, 379.0, 90.0, 18.0],
        &tr("doc15.hex", "Hex:"),
    );
    f.edit_on(
        1,
        "hex",
        [114.0, 376.0, 80.0, 24.0],
        &format!("#{}", to_hex(rgb)),
    );
    f.form.add("swatch", None, SWATCH, Kind::Canvas);
    f.button(
        "ok",
        [271.0, 47.0, 110.0, 29.0],
        &tr("common.ok", "OK"),
        true,
    );
    f.button(
        "cancel",
        [271.0, 86.0, 110.0, 29.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f.words = vec![
        ("new".into(), tr("doc15.new", "New")),
        ("current".into(), tr("doc15.current", "Current")),
        ("red".into(), tr("doc15.red", "Red:")),
        ("green".into(), tr("doc15.green", "Green:")),
        ("blue".into(), tr("doc15.blue", "Blue:")),
        ("hue".into(), tr("doc15.hue", "Hue:")),
        ("sat".into(), tr("doc15.sat", "Sat:")),
        ("lumL".into(), tr("doc15.lum", "Lum:")),
    ];
    // A grey has no hue: the field's marker starts at the hue the reference shows (2/3).
    let mut hsl = rgb_to_hsl(rgb);
    if hsl.1 <= 0.0 {
        hsl.0 = 2.0 / 3.0;
    }
    f.r15 = Some(Box::new(ColorsState {
        hsl,
        current,
        new: None,
        target,
    }));
    f.form.enable("ok", false);
    f
}

impl DocForm {
    fn colors(&self) -> Option<&ColorsState> {
        self.r15.as_deref()
    }

    /// Show `rgb` in the Custom tab's boxes (RGB or HSL model) and the hex box.
    fn colors_fill(&mut self, rgb: (u8, u8, u8)) {
        let hsl_model = self.sel("model") == Some(1);
        let vals = if hsl_model {
            let (h, s, l) = self.colors().map_or((0.0, 0.0, 0.0), |c| c.hsl);
            [
                (h * 255.0).round() as i32,
                (s * 255.0).round() as i32,
                (l * 255.0).round() as i32,
            ]
        } else {
            [i32::from(rgb.0), i32::from(rgb.1), i32::from(rgb.2)]
        };
        for (id, v) in ["c1", "c2", "c3"].into_iter().zip(vals) {
            self.form.set_num(id, v);
        }
        self.set_text("hex", &format!("#{}", to_hex(rgb)));
    }

    /// Colour `rgb` picked (`hsl`: its hue, saturation, luminance when known).
    fn colors_pick(&mut self, rgb: (u8, u8, u8), hsl: Option<(f32, f32, f32)>) {
        if let Some(c) = self.r15.as_mut() {
            c.new = Some(to_hex(rgb));
            let mut v = hsl.unwrap_or_else(|| rgb_to_hsl(rgb));
            // A grey keeps the hue the field showed.
            if v.1 <= 0.0 {
                v.0 = c.hsl.0;
            }
            c.hsl = v;
        }
        self.form.enable("ok", true);
        self.colors_fill(rgb);
    }

    /// A click on a round-15 canvas at (x, y) reference pixels inside it.
    pub(super) fn r15_canvas(&mut self, id: &str, x: f32, y: f32) -> bool {
        if !handles(self.kind) {
            return false;
        }
        match id {
            "hex6" => {
                let (lx, ly) = (HEX_CANVAS[0] + x, HEX_CANVAS[1] + y);
                let hit = cells().into_iter().find(|c| {
                    let (dx, dy) = ((lx - c.cx).abs(), (ly - c.cy).abs());
                    dx <= c.rx && dy <= c.ry && dy <= c.ry - (c.ry / 2.0) * (dx / c.rx)
                });
                if let Some(rgb) = hit.and_then(|c| parse_hex(c.hex)) {
                    self.colors_pick(rgb, None);
                }
                true
            }
            "spectrum" => {
                let h = (x / SPECTRUM[2]).clamp(0.0, 1.0);
                let s = 1.0 - (y / SPECTRUM[3]).clamp(0.0, 1.0);
                let l = self.colors().map_or(0.5, |c| c.hsl.2);
                // As the reference: a pick in the field of a black or white colour shows
                // the hue at half luminance.
                let l = if l <= 0.02 || l >= 0.98 { 0.5 } else { l };
                self.colors_pick(hsl_to_rgb(h, s, l), Some((h, s, l)));
                true
            }
            "lum" => {
                let top = 8.0;
                let h = LUM[3] - 2.0 * top;
                let l = 1.0 - ((y - top) / h).clamp(0.0, 1.0);
                let (hh, ss, _) = self.colors().map_or((0.0, 0.0, 0.5), |c| c.hsl);
                self.colors_pick(hsl_to_rgb(hh, ss, l), Some((hh, ss, l)));
                true
            }
            _ => false,
        }
    }

    /// A control of a round-15 dialog changed.
    pub(crate) fn r15_changed(&mut self, id: &str) {
        if self.kind == DocFormKind::ThemeColors {
            self.form.enable("reset", true);
            return;
        }
        if self.kind == DocFormKind::ThemeFonts {
            return;
        }
        let word = |f: &Self, k: &str| {
            f.words
                .iter()
                .find(|(n, _)| n == k)
                .map_or_else(String::new, |(_, v)| v.clone())
        };
        match id {
            "model" => {
                let hsl = self.sel("model") == Some(1);
                let names = if hsl {
                    ["hue", "sat", "lumL"]
                } else {
                    ["red", "green", "blue"]
                };
                for (lid, n) in ["c1L", "c2L", "c3L"].into_iter().zip(names) {
                    let w = word(self, n);
                    self.set_label(lid, &w);
                }
                let rgb = self.colors_rgb();
                self.colors_fill(rgb);
            }
            "c1" | "c2" | "c3" => {
                let v = [
                    self.form.num("c1").clamp(0, 255),
                    self.form.num("c2").clamp(0, 255),
                    self.form.num("c3").clamp(0, 255),
                ];
                if self.sel("model") == Some(1) {
                    let hsl = (
                        v[0] as f32 / 255.0,
                        v[1] as f32 / 255.0,
                        v[2] as f32 / 255.0,
                    );
                    let rgb = hsl_to_rgb(hsl.0, hsl.1, hsl.2);
                    if let Some(c) = self.r15.as_mut() {
                        c.new = Some(to_hex(rgb));
                        c.hsl = hsl;
                    }
                    self.set_text("hex", &format!("#{}", to_hex(rgb)));
                } else {
                    let rgb = (v[0] as u8, v[1] as u8, v[2] as u8);
                    if let Some(c) = self.r15.as_mut() {
                        c.new = Some(to_hex(rgb));
                        c.hsl = rgb_to_hsl(rgb);
                    }
                    self.set_text("hex", &format!("#{}", to_hex(rgb)));
                }
                self.form.enable("ok", true);
            }
            "hex" => {
                if let Some(rgb) = parse_hex(&self.text("hex")) {
                    let hsl = rgb_to_hsl(rgb);
                    if let Some(c) = self.r15.as_mut() {
                        c.new = Some(to_hex(rgb));
                        c.hsl = hsl;
                    }
                    let hsl_model = self.sel("model") == Some(1);
                    let vals = if hsl_model {
                        [
                            (hsl.0 * 255.0).round() as i32,
                            (hsl.1 * 255.0).round() as i32,
                            (hsl.2 * 255.0).round() as i32,
                        ]
                    } else {
                        [i32::from(rgb.0), i32::from(rgb.1), i32::from(rgb.2)]
                    };
                    for (id, v) in ["c1", "c2", "c3"].into_iter().zip(vals) {
                        self.form.set_num(id, v);
                    }
                    self.form.enable("ok", true);
                }
            }
            _ => {}
        }
    }

    /// The colour shown now (the new one, else the current one, else mid grey).
    fn colors_rgb(&self) -> (u8, u8, u8) {
        self.colors()
            .and_then(|c| c.new.as_deref().or(c.current.as_deref()))
            .and_then(parse_hex)
            .unwrap_or((128, 128, 128))
    }

    /// OK of a round-15 dialog.
    pub(super) fn r15_ok(&mut self) -> Option<FormOut> {
        if !handles(self.kind) {
            return None;
        }
        if let Some(out) = self.theme_parts_ok() {
            return Some(out);
        }
        let c = self.colors()?;
        let Some(hex) = c.new.clone() else {
            return Some(FormOut::Close);
        };
        let recent = json!({ "cmd": "ui.action", "id": format!("r15.recent:{hex}") });
        Some(match c.target.as_str() {
            "form" => FormOut::Run(
                vec![
                    recent,
                    json!({ "cmd": "ui.action", "id": format!("r15.colorBack:{hex}") }),
                ],
                true,
            ),
            _ => FormOut::Run(
                vec![recent, json!({ "cmd": "setPageColor", "color": hex })],
                true,
            ),
        })
    }

    /// More Colors… of a colour drop-down: the Colors dialog over this one.
    pub(super) fn r15_clicked(&mut self, id: &str) -> Option<FormOut> {
        if self.kind == DocFormKind::ThemeColors && id == "reset" {
            self.theme_colors_reset();
            return Some(FormOut::Redraw);
        }
        (id == "moreColors" && self.form.more_for.is_some())
            .then_some(FormOut::Stash(DocFormKind::Colors))
    }

    /// The colour of the drop-down More Colors… was chosen for (`""` = automatic).
    pub(crate) fn r15_more_color(&self) -> Option<String> {
        self.form.more_for.map(|id| self.form.color(id))
    }

    /// The Colors dialog's colour for the drop-down More Colors… was chosen for.
    pub(crate) fn r15_set_more_color(&mut self, hex: &str) {
        if let Some(id) = self.form.more_for.take() {
            self.form.set_color(id, hex);
            self.changed(id);
        }
    }

    /// More Colors… chosen in colour drop-down id (tests).
    #[cfg(test)]
    pub(crate) fn r15_more_for_test(&mut self, id: &'static str) {
        self.form.more_for = Some(id);
    }

    /// Colour of drop-down id (tests).
    #[cfg(test)]
    pub(crate) fn r15_color_for_test(&self, id: &str) -> String {
        self.form.color(id)
    }

    /// Pick colour hex as the Standard tab would (scripted captures).
    pub(crate) fn r15_pick(&mut self, hex: &str) {
        if let Some(rgb) = parse_hex(hex) {
            self.colors_pick(rgb, None);
        }
    }

    /// The colour picked (tests).
    #[cfg(test)]
    pub(crate) fn r15_new_for_test(&self) -> Option<String> {
        self.colors().and_then(|c| c.new.clone())
    }
}

/// Paint a round-15 canvas; `false` when `id` is not one.
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
    match id {
        "tcSample" => {
            paint_theme_colors_sample(f, r, p, font);
            return true;
        }
        "tfSample" => {
            paint_theme_fonts_sample(f, r, p, font);
            return true;
        }
        _ => {}
    }
    let Some(st) = f.colors() else {
        return false;
    };
    match id {
        "hex6" => paint_hexagon(st, r, p),
        "spectrum" => paint_spectrum(st, r, p),
        "lum" => paint_lum(st, r, p),
        "swatch" => paint_swatch(f, st, r, p, font),
        _ => return false,
    }
    true
}

fn rgba(h: &str) -> Rgba {
    let (r, g, b) = parse_hex(h).unwrap_or((0, 0, 0));
    Rgba(r, g, b, 255)
}

/// A pointy-top hexagon around (cx, cy) with half width `rx` and half height `ry`.
fn hexagon(cx: f32, cy: f32, rx: f32, ry: f32) -> Vec<PathSeg> {
    vec![
        PathSeg::M(cx, cy - ry),
        PathSeg::L(cx + rx, cy - ry / 2.0),
        PathSeg::L(cx + rx, cy + ry / 2.0),
        PathSeg::L(cx, cy + ry),
        PathSeg::L(cx - rx, cy + ry / 2.0),
        PathSeg::L(cx - rx, cy - ry / 2.0),
        PathSeg::Z,
    ]
}

fn paint_hexagon(st: &ColorsState, r: [f32; 4], p: &mut dyn Painter) {
    let k = r[2] / HEX_CANVAS[2];
    let at = |x: f32, y: f32| {
        (
            r[0] + (x - HEX_CANVAS[0]) * k,
            r[1] + (y - HEX_CANVAS[1]) * k,
        )
    };
    let chosen = st.new.as_deref().or(st.current.as_deref());
    let mut sel = None;
    for c in cells() {
        let (x, y) = at(c.cx, c.cy);
        // A hair larger so neighbouring cells meet without seams.
        p.path(
            &hexagon(x, y, c.rx * k + 0.5, c.ry * k + 0.5),
            Some(rgba(c.hex)),
            None,
        );
        if chosen.is_some_and(|h| h.eq_ignore_ascii_case(c.hex)) && sel.is_none() {
            sel = Some((x, y, c.rx * k, c.ry * k));
        }
    }
    // The chosen cell: a black outline around a white one, as the reference.
    if let Some((x, y, rx, ry)) = sel {
        p.path(
            &hexagon(x, y, rx + 1.0, ry + 1.0),
            None,
            Some((Rgba(0, 0, 0, 255), 2.0)),
        );
        p.path(
            &hexagon(x, y, rx - 1.0, ry - 1.0),
            None,
            Some((Rgba(255, 255, 255, 255), 1.5)),
        );
    }
}

fn paint_spectrum(st: &ColorsState, r: [f32; 4], p: &mut dyn Painter) {
    // Hue left → right, saturation top → bottom at half luminance, in 3 px blocks.
    let step = 3.0_f32;
    let (cols, rows) = ((r[2] / step).ceil() as usize, (r[3] / step).ceil() as usize);
    for j in 0..rows {
        let s = 1.0 - (j as f32 + 0.5) / rows as f32;
        for i in 0..cols {
            let h = (i as f32 + 0.5) / cols as f32;
            let (cr, cg, cb) = hsl_to_rgb(h, s, 0.5);
            let x = r[0] + i as f32 * step;
            let y = r[1] + j as f32 * step;
            p.fill_rect(
                x,
                y,
                step.min(r[0] + r[2] - x),
                step.min(r[1] + r[3] - y),
                Rgba(cr, cg, cb, 255),
            );
        }
    }
    // Cross-hair at the hue and saturation, in black (the reference's marker).
    let (h, s, _) = st.hsl;
    let cx = (r[0] + h * r[2]).round();
    let cy = (r[1] + (1.0 - s) * r[3]).round();
    let ink = Rgba(0, 0, 0, 255);
    let k = r[2] / SPECTRUM[2];
    let (a, b) = ((3.0 * k).round(), (8.0 * k).round());
    for (x, y, w, hh) in [
        (cx - b, cy - 1.0, b - a, 2.0),
        (cx + a, cy - 1.0, b - a, 2.0),
        (cx - 1.0, cy - b, 2.0, b - a),
        (cx - 1.0, cy + a, 2.0, b - a),
    ] {
        p.push_clip(r[0], r[1], r[2], r[3]);
        p.fill_rect(x, y, w, hh, ink);
        p.pop_clip();
    }
}

fn paint_lum(st: &ColorsState, r: [f32; 4], p: &mut dyn Painter) {
    let k = r[2] / LUM[2];
    let (bx, bw) = (r[0] + 4.0 * k, (12.0 * k).round());
    let top = r[1] + 8.0 * k;
    let h = r[3] - 16.0 * k;
    let (hh, ss, ll) = st.hsl;
    let n = (h / 2.0).ceil() as usize;
    for j in 0..n {
        let l = 1.0 - (j as f32 + 0.5) / n as f32;
        let (cr, cg, cb) = hsl_to_rgb(hh, ss, l);
        let y = top + j as f32 * 2.0;
        p.fill_rect(bx, y, bw, 2.0_f32.min(top + h - y), Rgba(cr, cg, cb, 255));
    }
    // The luminance arrow to the bar's right.
    let ay = (top + (1.0 - ll) * h).round();
    let ax = bx + bw + 3.0 * k;
    let a = 6.0 * k;
    p.path(
        &[
            PathSeg::M(ax, ay),
            PathSeg::L(ax + a, ay - a * 0.75),
            PathSeg::L(ax + a, ay + a * 0.75),
            PathSeg::Z,
        ],
        Some(Rgba(0, 0, 0, 255)),
        None,
    );
}

fn paint_swatch(f: &DocForm, st: &ColorsState, r: [f32; 4], p: &mut dyn Painter, font: &Font<'_>) {
    let k = r[2] / SWATCH[2];
    let word = |n: &str| {
        f.words
            .iter()
            .find(|(w, _)| w == n)
            .map_or("", |(_, v)| v.as_str())
    };
    let ink = Rgba(0, 0, 0, 255);
    let line = font.size * 1.33;
    let cw = |s: &str| font.size * 0.5 * s.chars().count() as f32;
    let mid = r[0] + 46.0 * k;
    p.text_font(
        word("new"),
        (mid - cw(word("new")) / 2.0).round(),
        (r[1] + 4.0 * k).round(),
        font,
        ink,
    );
    let (bx, by, bw, bh) = (
        (r[0] + 14.0 * k).round(),
        (r[1] + 27.0 * k).round(),
        (65.0 * k).round(),
        (65.0 * k).round(),
    );
    p.fill_rect(bx, by, bw, bh, Rgba(0x64, 0x64, 0x64, 255));
    let half = (bh / 2.0).round();
    let face = Rgba(0xf0, 0xf0, 0xf0, 255);
    // The Custom tab shows its boxes' colour as New before anything is picked.
    let new = match st.new.as_deref() {
        Some(h) => rgba(h),
        None if f.form.tab == 1 => {
            let (r0, g0, b0) = f.colors_rgb();
            Rgba(r0, g0, b0, 255)
        }
        None => face,
    };
    let cur = st.current.as_deref().map_or(face, rgba);
    p.fill_rect(bx + 1.0, by + 1.0, bw - 2.0, half - 1.0, new);
    p.fill_rect(bx + 1.0, by + half, bw - 2.0, bh - half - 1.0, cur);
    p.text_font(
        word("current"),
        (mid - cw(word("current")) / 2.0).round(),
        (by + bh + 8.0 * k).round().min(r[1] + r[3] - line),
        font,
        ink,
    );
}

// ------------------------------------------------- Create New Theme Colors / Theme Fonts

/// Rows of Create New Theme Colors: control id, label key, en-US label (in §20.1.6.2
/// clrScheme order).
const THEME_ROWS: [(&str, &str, &str); 12] = [
    ("tc0", "doc15.tcDark1", "Text/Background - Dark 1"),
    ("tc1", "doc15.tcLight1", "Text/Background - Light 1"),
    ("tc2", "doc15.tcDark2", "Text/Background - Dark 2"),
    ("tc3", "doc15.tcLight2", "Text/Background - Light 2"),
    ("tc4", "doc15.tcAccent1", "Accent 1"),
    ("tc5", "doc15.tcAccent2", "Accent 2"),
    ("tc6", "doc15.tcAccent3", "Accent 3"),
    ("tc7", "doc15.tcAccent4", "Accent 4"),
    ("tc8", "doc15.tcAccent5", "Accent 5"),
    ("tc9", "doc15.tcAccent6", "Accent 6"),
    ("tc10", "doc15.tcHyperlink", "Hyperlink"),
    ("tc11", "doc15.tcFollowed", "Followed Hyperlink"),
];

/// Label ids of [`THEME_ROWS`].
const THEME_LABELS: [&str; 12] = [
    "tc0L", "tc1L", "tc2L", "tc3L", "tc4L", "tc5L", "tc6L", "tc7L", "tc8L", "tc9L", "tc10L",
    "tc11L",
];

/// Scheme used when the document has no theme colours (dk1 lt1 dk2 lt2 accent1–6 hlink
/// folHlink; the default theme's).
const FALLBACK_SCHEME: [&str; 12] = [
    "000000", "FFFFFF", "44546A", "E7E6E6", "4472C4", "ED7D31", "A5A5A5", "FFC000", "5B9BD5",
    "70AD47", "0563C1", "954F72",
];

/// Create New Theme Colors (612 × 657 measured): the twelve scheme colours with their
/// drop-downs, a light / dark sample, the name; Reset, Save, Cancel.
fn open_theme_colors(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::ThemeColors,
        &tr("doc15.tcTitle", "Create New Theme Colors"),
        (612.0, 657.0),
        vec![],
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
    let scheme: Vec<String> = (0..12)
        .map(|i| {
            c.extra["r15"]["scheme"][i]
                .as_str()
                .map_or_else(|| FALLBACK_SCHEME[i].to_owned(), str::to_owned)
        })
        .collect();
    f.group(
        "tcGroup",
        [20.0, 62.0, 302.0, 495.0],
        &tr("doc15.tcThemeColors", "Theme colors"),
    );
    for (i, (id, key, en)) in THEME_ROWS.into_iter().enumerate() {
        let y = 109.0 + 38.0 * i as f32;
        f.label(THEME_LABELS[i], [38.0, y - 10.0, 190.0, 18.0], &tr(key, en));
        f.form.add(
            id,
            None,
            [233.0, y - 14.0, 64.0, 29.0],
            Kind::ColorPick {
                hex: scheme[i].clone(),
                palette: palette.clone(),
            },
        );
    }
    f.group(
        "tcSampleG",
        [337.0, 62.0, 263.0, 253.0],
        &tr("doc15.sample", "Sample"),
    );
    f.canvas("tcSample", [356.0, 93.0, 219.0, 205.0]);
    f.label(
        "nameL",
        [29.0, 576.0, 60.0, 18.0],
        &tr("doc15.name", "Name:"),
    );
    let name = c.extra["r15"]["customName"]
        .as_str()
        .unwrap_or("Custom 1")
        .to_owned();
    f.edit("name", [89.0, 571.0, 509.0, 28.0], &name);
    f.button(
        "reset",
        [9.0, 613.0, 106.0, 32.0],
        &tr("doc15.reset", "Reset"),
        false,
    );
    f.button(
        "ok",
        [375.0, 613.0, 106.0, 32.0],
        &tr("doc15.save", "Save"),
        true,
    );
    f.button(
        "cancel",
        [497.0, 613.0, 106.0, 32.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f.form.enable("reset", false);
    let name_word = ("nameText".to_owned(), name.clone());
    f.words = scheme
        .iter()
        .enumerate()
        .map(|(i, h)| (format!("tc{i}"), h.clone()))
        .chain([
            ("text".to_owned(), tr("doc15.sampleText", "Text")),
            ("link".to_owned(), tr("doc15.tcHyperlink", "Hyperlink")),
            name_word,
        ])
        .collect();
    f.form.focus("name");
    f.form.select_all("name");
    f
}

/// Create New Theme Fonts (671 × 283 measured): heading and body font, a sample, the name.
fn open_theme_fonts(s: &Strings, c: &FormContext) -> DocForm {
    let tr = |k: &str, fb: &str| t(s, k, fb);
    let mut f = DocForm::new(
        DocFormKind::ThemeFonts,
        &tr("doc15.tfTitle", "Create New Theme Fonts"),
        (671.0, 283.0),
        vec![],
    );
    let mut fonts: Vec<String> = c.extra["r15"]["fonts"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let major = c.extra["r15"]["major"].as_str().unwrap_or("Calibri Light");
    let minor = c.extra["r15"]["minor"].as_str().unwrap_or("Calibri");
    for name in [major, minor] {
        if !fonts.iter().any(|x| x == name) {
            fonts.push(name.to_owned());
        }
    }
    fonts.sort_by_key(|x| x.to_lowercase());
    let at = |n: &str| fonts.iter().position(|x| x == n).unwrap_or(0);
    f.label(
        "majorL",
        [9.0, 60.0, 200.0, 18.0],
        &tr("doc15.headingFont", "Heading font:"),
    );
    f.combo("major", [9.0, 84.0, 298.0, 32.0], fonts.clone(), at(major));
    f.label(
        "minorL",
        [9.0, 130.0, 200.0, 18.0],
        &tr("doc15.bodyFont", "Body font:"),
    );
    f.combo("minor", [9.0, 153.0, 298.0, 32.0], fonts.clone(), at(minor));
    f.label(
        "sampleL",
        [322.0, 60.0, 100.0, 18.0],
        &tr("doc15.sample", "Sample"),
    );
    f.canvas("tfSample", [321.0, 83.0, 311.0, 107.0]);
    f.label(
        "nameL",
        [29.0, 207.0, 60.0, 18.0],
        &tr("doc15.name", "Name:"),
    );
    let name = c.extra["r15"]["customName"]
        .as_str()
        .unwrap_or("Custom 1")
        .to_owned();
    f.edit("name", [84.0, 203.0, 578.0, 28.0], &name);
    f.button(
        "ok",
        [434.0, 238.0, 106.0, 32.0],
        &tr("doc15.save", "Save"),
        true,
    );
    f.button(
        "cancel",
        [556.0, 238.0, 106.0, 32.0],
        &tr("common.cancel", "Cancel"),
        false,
    );
    f.words = vec![
        ("heading".into(), tr("doc15.sampleHeading", "Heading")),
        (
            "body".into(),
            tr(
                "doc15.sampleBody",
                "Body text body text body text. Body text body text.",
            ),
        ),
        ("nameText".into(), name.clone()),
    ];
    f.form.focus("name");
    f.form.select_all("name");
    f
}

impl DocForm {
    /// Save of Create New Theme Colors / Fonts: the theme with the new parts (one step).
    fn theme_parts_ok(&self) -> Option<FormOut> {
        let name = self.text("name");
        let cmd = match self.kind {
            DocFormKind::ThemeColors => {
                let colors: Vec<String> = THEME_ROWS
                    .iter()
                    .enumerate()
                    .map(|(i, (id, ..))| {
                        let h = self.form.color(id);
                        if parse_hex(&h).is_some() {
                            h.to_uppercase()
                        } else {
                            FALLBACK_SCHEME[i].to_owned()
                        }
                    })
                    .collect();
                json!({ "cmd": "applyThemeParts", "colors": colors, "name": name })
            }
            DocFormKind::ThemeFonts => json!({
                "cmd": "applyThemeParts", "major": self.text("major"),
                "minor": self.text("minor"), "name": name,
            }),
            _ => return None,
        };
        Some(FormOut::Run(vec![cmd], true))
    }

    /// Reset: the colours the dialog opened with.
    fn theme_colors_reset(&mut self) {
        for (id, ..) in THEME_ROWS {
            let v = self
                .words
                .iter()
                .find(|(k, _)| k == id)
                .map(|(_, v)| v.clone());
            if let Some(v) = v {
                self.form.set_color(id, &v);
            }
        }
        self.form.enable("reset", false);
    }

    /// Colour of drop-down `id` (tests).
    #[cfg(test)]
    pub(crate) fn r15_set_color_for_test(&mut self, id: &str, hex: &str) {
        self.form.set_color(id, hex);
    }
}

/// A dark and a light panel of the scheme: "Text", a bar chart in the six accents and the
/// two hyperlink colours (own sample drawing).
fn paint_theme_colors_sample(f: &DocForm, r: [f32; 4], p: &mut dyn Painter, font: &Font<'_>) {
    let col = |i: usize| {
        let h = f.form.color(THEME_ROWS[i].0);
        parse_hex(&h).map_or(Rgba(0x80, 0x80, 0x80, 255), |(a, b, c)| Rgba(a, b, c, 255))
    };
    let word = |k: &str| {
        f.words
            .iter()
            .find(|(n, _)| n == k)
            .map_or("", |(_, v)| v.as_str())
    };
    let k = r[2] / 219.0;
    let half = (r[2] / 2.0).round();
    // Dark panel (dk2 behind lt1 text), light panel (lt2 behind dk1 text).
    for (pi, (bg, ink)) in [(col(2), col(1)), (col(3), col(0))].into_iter().enumerate() {
        let x0 = r[0] + pi as f32 * half;
        p.fill_rect(x0, r[1], half, r[3], bg);
        let bold = Font {
            weight: 700,
            ..*font
        };
        let tw = font.size * 0.55 * word("text").chars().count() as f32;
        p.text_font(
            word("text"),
            (x0 + (half - tw) / 2.0).round(),
            r[1] + 12.0 * k,
            &bold,
            ink,
        );
        // Bars in the six accents on a base line.
        let base = r[1] + 136.0 * k;
        let heights = [14.0, 24.0, 36.0, 30.0, 22.0, 10.0];
        let bw = 10.0 * k;
        let bx = x0 + (half - 6.0 * bw) / 2.0;
        for (j, hgt) in heights.iter().enumerate() {
            p.fill_rect(
                bx + j as f32 * bw,
                base - hgt * k,
                bw - 1.0,
                hgt * k,
                col(4 + j),
            );
        }
        p.fill_rect(x0 + 10.0 * k, base, half - 20.0 * k, k.max(1.0), ink);
        // Hyperlink / followed hyperlink, underlined.
        let underline = Font {
            underline: true,
            ..*font
        };
        let lw = font.size * 0.55 * word("link").chars().count() as f32;
        for (li, c) in [col(10), col(11)].into_iter().enumerate() {
            p.text_font(
                word("link"),
                (x0 + (half - lw) / 2.0).round(),
                r[1] + (154.0 + 20.0 * li as f32) * k,
                &underline,
                c,
            );
        }
    }
}

/// The heading and body fonts chosen, in a white box framed grey.
fn paint_theme_fonts_sample(f: &DocForm, r: [f32; 4], p: &mut dyn Painter, font: &Font<'_>) {
    p.fill_rect(r[0], r[1], r[2], r[3], Rgba(0xa0, 0xa0, 0xa0, 255));
    p.fill_rect(
        r[0] + 1.0,
        r[1] + 1.0,
        r[2] - 2.0,
        r[3] - 2.0,
        Rgba(255, 255, 255, 255),
    );
    let word = |k: &str| {
        f.words
            .iter()
            .find(|(n, _)| n == k)
            .map_or_else(String::new, |(_, v)| v.clone())
    };
    let major = f.text("major");
    let minor = f.text("minor");
    let heading = Font {
        family: &major,
        size: (font.size * 1.9).round(),
        weight: 400,
        italic: false,
        underline: false,
    };
    let body = Font {
        family: &minor,
        size: (font.size * 1.25).round(),
        weight: 400,
        italic: false,
        underline: false,
    };
    p.push_clip(r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0);
    let ink = Rgba(0, 0, 0, 255);
    p.text_font(&word("heading"), r[0] + 14.0, r[1] + 8.0, &heading, ink);
    p.text_font(
        &word("body"),
        r[0] + 14.0,
        r[1] + 8.0 + heading.size * 1.4,
        &body,
        ink,
    );
    p.pop_clip();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hexagon_has_127_colours_and_17_greys() {
        let n: usize = HEX_ROWS.iter().map(|r| r.len()).sum();
        assert_eq!(n, 127);
        assert_eq!(cells().len(), 127 + 2 + 8 + 7);
        assert_eq!(HEX_ROWS[6][6], "FFFFFF");
    }

    #[test]
    fn hsl_round_trips() {
        for c in [(255, 0, 0), (0, 128, 255), (51, 204, 51), (128, 128, 128)] {
            let (h, s, l) = rgb_to_hsl(c);
            let back = hsl_to_rgb(h, s, l);
            assert!(
                (i32::from(back.0) - i32::from(c.0)).abs() <= 1
                    && (i32::from(back.1) - i32::from(c.1)).abs() <= 1
                    && (i32::from(back.2) - i32::from(c.2)).abs() <= 1,
                "{c:?} -> {back:?}"
            );
        }
        assert_eq!(parse_hex("#c55a11"), Some((0xc5, 0x5a, 0x11)));
        assert_eq!(parse_hex("12345"), None);
    }
}
