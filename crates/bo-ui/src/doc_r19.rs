//! BlueDoc parity round 19 (docs/ui/REPORT.md "Vòng 109"): Picture Format › Picture Styles —
//! the Quick Styles gallery, Picture Border ▾ (colours, weights, dashes), Picture Effects ▾
//! (Preset, Shadow, Reflection, Glow, Soft Edges, Bevel, 3-D Rotation — each a second menu
//! at the same button) and Artistic Effects ▾. The commands run on the engine
//! (`bo_engine::doc_picstyle`). A child module of the shell.
use super::DocWindow;
use crate::popup::{Choice, Menu, MenuItem};
use crate::ribbon::RibbonHit;
use bo_engine::doc_picstyle as ps;

#[cfg(test)]
#[path = "doc_r19_tests.rs"]
mod tests;

/// Colours of Picture Border (standard colours): (hex, key, English); the round-18 keys.
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

/// Dashes (§20.1.10.48): (value, key, English) — the round-18 keys.
const DASHES: [(&str, &str, &str); 6] = [
    ("solid", "doc18.dSolid", "Solid"),
    ("sysDot", "doc18.dRoundDot", "Round Dot"),
    ("sysDash", "doc18.dSquareDot", "Square Dot"),
    ("dash", "doc18.dDash", "Dash"),
    ("dashDot", "doc18.dDashDot", "Dash Dot"),
    ("lgDash", "doc18.dLongDash", "Long Dash"),
];

/// Weights of Picture Border (points).
const WEIGHTS: [&str; 9] = ["0.25", "0.5", "0.75", "1", "1.5", "2.25", "3", "4.5", "6"];

/// The Picture Effects categories: (kind, key, English).
const KINDS: [(&str, &str, &str); 7] = [
    ("preset", "doc19.kPreset", "Preset"),
    ("shadow", "doc19.kShadow", "Shadow"),
    ("reflection", "doc19.kReflection", "Reflection"),
    ("glow", "doc19.kGlow", "Glow"),
    ("softEdge", "doc19.kSoftEdges", "Soft Edges"),
    ("bevel", "doc19.kBevel", "Bevel"),
    ("rotation", "doc19.kRotation", "3-D Rotation"),
];

/// Commands of round 19 that work on a selected picture.
pub(crate) const IDS: [&str; 4] = [
    "picture.border",
    "picture.effects",
    "picture.artisticEffects",
    "picture.quickStyles",
];

/// Words of a camel-case preset name ("perspectiveRelaxedModerately" → "Perspective
/// Relaxed Moderately").
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

impl DocWindow {
    fn r19_t(&self, key: &str, fallback: &str) -> String {
        let v = self.strings.get(key);
        if v == key {
            fallback.to_owned()
        } else {
            v.to_owned()
        }
    }

    /// A round-19 command; `None` when `id` is not one.
    pub(super) fn r19_action(&mut self, id: &str) -> Option<bool> {
        Some(match id {
            _ if IDS.contains(&id) => {
                let sid = IDS.iter().copied().find(|i| *i == id).unwrap_or(IDS[0]);
                self.r19_menu(sid, None)
            }
            _ if id.starts_with("r19.fxmenu:") => {
                let kind = id["r19.fxmenu:".len()..].to_owned();
                self.r19_menu("picture.effects", Some(&kind))
            }
            _ if id.starts_with("r19.bc:") => self.r19_run(&serde_json::json!({
                "cmd": "pictureBorder", "color": &id["r19.bc:".len()..]
            })),
            _ if id.starts_with("r19.bw:") => {
                let w: f64 = id["r19.bw:".len()..].parse().unwrap_or(1.0);
                self.r19_run(&serde_json::json!({ "cmd": "pictureBorder", "weight": w }))
            }
            _ if id.starts_with("r19.bd:") => self.r19_run(&serde_json::json!({
                "cmd": "pictureBorder", "dash": &id["r19.bd:".len()..]
            })),
            _ if id.starts_with("r19.fx:") => {
                let rest = &id["r19.fx:".len()..];
                let (kind, preset) = rest.split_once('=').unwrap_or((rest, "none"));
                self.r19_run(&serde_json::json!({
                    "cmd": "pictureEffect", "kind": kind, "preset": preset
                }))
            }
            _ if id.starts_with("r19.art:") => self.r19_run(&serde_json::json!({
                "cmd": "pictureArtistic", "name": &id["r19.art:".len()..]
            })),
            _ if id.starts_with("r19.quick:") => {
                let i: u64 = id["r19.quick:".len()..].parse().unwrap_or(0);
                self.r19_run(&serde_json::json!({ "cmd": "pictureQuickStyle", "index": i }))
            }
            _ => return None,
        })
    }

    /// The window's view: `print`, `web`, `draft` or `read`.
    pub fn view_mode_name(&self) -> &'static str {
        self.view_mode
    }

    /// Take the view of another window (View › New Window starts in it, round 19).
    pub fn set_view_mode_name(&mut self, mode: &'static str) {
        self.view_mode = mode;
    }

    /// View › New Window: each window keeps its own view (Print Layout / Web Layout /
    /// Draft), as the reference. When the document's engine comes to this window, its view
    /// goes back on the engine; `true` when the view changed.
    pub fn restore_view_mode(&mut self) -> bool {
        use bo_engine::views::ViewMode;
        let want = match self.view_mode {
            "web" => ViewMode::Web,
            "draft" => ViewMode::Draft,
            "print" => ViewMode::Print,
            _ => return false,
        };
        let now = self.view.engine().view_layout().mode;
        if now == want || !matches!(now, ViewMode::Print | ViewMode::Web | ViewMode::Draft) {
            return false;
        }
        let id = format!("view.layout.{}", self.view_mode);
        self.action(&id)
    }

    /// Run a picture style command on the selected picture.
    fn r19_run(&mut self, v: &serde_json::Value) -> bool {
        self.menu = None;
        let ok = self.view.picture_command(v);
        self.sync_context();
        ok
    }

    /// The selected picture's current style (border, effects, artistic effect).
    fn r19_current(&self) -> bo_model::picture::PictureFormat {
        self.view
            .engine()
            .session()
            .and_then(bo_edit::Session::tools_picture)
            .and_then(|i| i.format.map(|f| *f))
            .unwrap_or_default()
    }

    /// Open the drop-down of `id` (`kind`: the Picture Effects category shown).
    fn r19_menu(&mut self, id: &'static str, kind: Option<&str>) -> bool {
        let cur = self.r19_current();
        let item = |label: String, cmd: String, checked: bool| MenuItem {
            label,
            choice: Choice::Command(cmd),
            checked,
            dim: false,
        };
        let mut items = Vec::new();
        let fx = cur.effects.as_deref().cloned().unwrap_or_default();
        match (id, kind) {
            ("picture.border", _) => {
                let line = cur.line.as_ref();
                items.push(item(
                    self.r19_t("doc18.noOutline", "No Outline"),
                    "r19.bc:none".into(),
                    line.is_none(),
                ));
                for (hex, key, en) in COLORS {
                    let on = line.is_some_and(|l| format!("{:06X}", l.color) == hex);
                    items.push(item(self.r19_t(key, en), format!("r19.bc:{hex}"), on));
                }
                for w in WEIGHTS {
                    let on = line.is_some_and(|l| {
                        (l.width as f64 / 12_700.0 - w.parse::<f64>().unwrap_or(0.0)).abs() < 0.01
                    });
                    items.push(item(format!("{w} pt"), format!("r19.bw:{w}"), on));
                }
                for (dash, key, en) in DASHES {
                    let on = line.is_some_and(|l| l.dash.as_deref().unwrap_or("solid") == dash);
                    items.push(item(self.r19_t(key, en), format!("r19.bd:{dash}"), on));
                }
            }
            ("picture.effects", None) => {
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
                    items.push(item(
                        format!("{} \u{25B8}", self.r19_t(key, en)),
                        format!("r19.fxmenu:{k}"),
                        on,
                    ));
                }
            }
            ("picture.effects", Some(k)) => {
                let none = match k {
                    "preset" => self.r19_t("doc19.noPreset", "No Presets"),
                    "shadow" => self.r19_t("doc19.noShadow", "No Shadow"),
                    "reflection" => self.r19_t("doc19.noReflection", "No Reflection"),
                    "glow" => self.r19_t("doc19.noGlow", "No Glow"),
                    "softEdge" => self.r19_t("doc19.noSoftEdges", "No Soft Edges"),
                    "bevel" => self.r19_t("doc19.noBevel", "No Bevel"),
                    _ => self.r19_t("doc19.noRotation", "No Rotation"),
                };
                items.push(item(none, format!("r19.fx:{k}=none"), false));
                let mut add = |label: String, preset: String, on: bool| {
                    items.push(item(label, format!("r19.fx:{k}={preset}"), on));
                };
                match k {
                    "preset" => {
                        for i in 0..ps::PRESETS.len() {
                            add(
                                format!("{} {}", self.r19_t("doc19.kPreset", "Preset"), i + 1),
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
                            add(
                                words(name)
                                    .replace(" 4", ", 4 pt offset")
                                    .replace(" 8", ", 8 pt offset"),
                                name.to_owned(),
                                false,
                            );
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
            ("picture.artisticEffects", _) => {
                items.push(item(
                    self.r19_t("doc19.artNone", "None"),
                    "r19.art:none".into(),
                    cur.artistic.is_none(),
                ));
                for name in ps::artistic_names() {
                    let on = cur.artistic.as_ref().is_some_and(|a| a.name == name);
                    let label = words(name.trim_start_matches("artistic"));
                    items.push(item(label, format!("r19.art:{name}"), on));
                }
            }
            _ => {
                for i in 0..ps::QUICK_STYLES.len() {
                    items.push(item(
                        format!(
                            "{} {}",
                            self.r19_t("doc19.quickStyle", "Picture Style"),
                            i + 1
                        ),
                        format!("r19.quick:{i}"),
                        false,
                    ));
                }
            }
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
