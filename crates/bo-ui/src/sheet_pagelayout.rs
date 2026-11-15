//! Page Layout › Themes of BlueSheet (round 27): Themes, Colors and Fonts menus over the
//! BlueOffice theme set (`bo_edit::design::THEMES`, own names, palettes and font pairs, the
//! same set BlueDoc offers), applied as a theme part (ECMA-376 Part 1, §14.2.7; §20.1.6.2
//! clrScheme, §20.1.4.1.18 fontScheme) through the engine's `applyThemeFile`; Browse for
//! Themes… applies a `.thmx` (or the theme of another document). The theme part is saved
//! with the workbook.

use super::SheetWindow;
use base64::Engine as _;
use bo_edit::design::{ThemeDef, THEMES};
use bo_render::Rgba;
use serde_json::json;

/// Own drawings for the Themes group buttons (24-unit grid).
pub(super) fn add_icons(icons: &mut crate::icons::Icons) {
    let c = |h: u32| Some(Rgba((h >> 16) as u8, (h >> 8) as u8, h as u8, 255));
    icons.add_solid(
        "sheet.themes",
        &[
            ("M3 4h18v16h-18Z", c(0xFF_FF_FF)),
            ("M3 4h18v16h-18Z", None),
            ("M5 7h8v2h-8Z", c(0x1F_6F_B2)),
            ("M5 11h14v7h-14Z", c(0xD6_E4_F2)),
            ("M14 7h5v2h-5Z", c(0xF2_A5_41)),
        ],
    );
    icons.add_solid(
        "sheet.themeColors",
        &[
            ("M3 5h4v14h-4Z", c(0x1F_6F_B2)),
            ("M8 5h4v14h-4Z", c(0x2B_A3_A0)),
            ("M13 5h4v14h-4Z", c(0xF2_A5_41)),
            ("M18 5h3v14h-3Z", c(0xD9_53_4F)),
        ],
    );
    icons.add_solid(
        "sheet.themeFonts",
        &[("M4 19l5-14h2l5 14M6 14h8", None), ("M15 19h6", None)],
    );
    icons.add_solid(
        "sheet.themeEffects",
        &[
            ("M12 4a8 8 0 1 0 0.1 0Z", c(0xD6_E4_F2)),
            ("M12 4a8 8 0 1 0 0.1 0Z", None),
        ],
    );
}

/// The theme `t` as a `.thmx` package, base64 (as hosts send theme files).
fn thmx_b64(t: &ThemeDef) -> Option<String> {
    let xml = bo_edit::design::theme_xml(t);
    let pkg = bo_engine::variant::thmx_package(xml.as_bytes()).ok()?;
    Some(base64::engine::general_purpose::STANDARD.encode(pkg))
}

impl SheetWindow {
    /// The BlueOffice theme the workbook uses now (by the theme part's name), if any.
    fn current_theme(&mut self) -> Option<&'static ThemeDef> {
        let b64 = self
            .view
            .query(json!({ "cmd": "saveThemeFile" }))
            .and_then(|v| v.get("file").and_then(|f| f.as_str().map(str::to_owned)));
        let _ = self.view.take_error();
        let xml = bo_engine::theme_file::theme_xml_of_b64(&b64?)?;
        let text = String::from_utf8_lossy(&xml);
        THEMES
            .iter()
            .find(|t| text.contains(&format!("name=\"{}\"", t.name)))
    }

    /// Themes group commands; `None` for other ids.
    pub(super) fn theme_action(&mut self, id: &str) -> Option<bool> {
        let (kind, arg) = id.split_once('.')?;
        if !matches!(kind, "theme" | "themeColors" | "themeFonts") {
            return None;
        }
        if id == "theme.browse" {
            self.pick = Some("theme");
            self.requests.push(crate::Request::PickMergeData);
            return Some(true);
        }
        let chosen = THEMES.iter().find(|t| t.id == arg)?;
        let def = match kind {
            "theme" => *chosen,
            // Colors / Fonts keep the other half of the current theme (the default font pair
            // or palette when the workbook's theme is not a BlueOffice one).
            "themeColors" => {
                let cur = self.current_theme();
                ThemeDef {
                    colors: chosen.colors,
                    major: cur.map_or("Calibri Light", |c| c.major),
                    minor: cur.map_or("Calibri", |c| c.minor),
                    ..*chosen
                }
            }
            _ => {
                let cur = self.current_theme();
                ThemeDef {
                    major: chosen.major,
                    minor: chosen.minor,
                    ..*cur.unwrap_or(chosen)
                }
            }
        };
        let Some(file) = thmx_b64(&def) else {
            return Some(false);
        };
        let ok = self
            .view
            .run(json!({ "cmd": "applyThemeFile", "file": file }));
        if !ok {
            self.status = self.view.take_error().unwrap_or_default();
        }
        // Cells drawn with theme colours and fonts change.
        self.view.invalidate();
        Some(ok)
    }

    /// A theme file picked by Browse for Themes….
    pub(super) fn picked_theme(&mut self, bytes: &[u8]) -> bool {
        let file = base64::engine::general_purpose::STANDARD.encode(bytes);
        let ok = self
            .view
            .run(json!({ "cmd": "applyThemeFile", "file": file }));
        if !ok {
            self.status = self.view.take_error().unwrap_or_default();
        }
        self.view.invalidate();
        ok
    }
}
