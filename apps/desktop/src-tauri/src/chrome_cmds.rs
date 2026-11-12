//! Window chrome of the desktop apps (ADR-0016): the web title bar is the window caption.
//!
//! On Windows the windows have no system frame; the web UI draws the caption buttons and asks
//! for the snap-layouts overlay over its maximise button ([`chrome_set_max_button`]), the system
//! menu ([`chrome_system_menu`]) and the Mica backdrop ([`chrome_set_backdrop`], Windows 11).
//! macOS and Linux keep their native frame (the web title bar then draws no window buttons).

use serde::{Deserialize, Serialize};
use tauri::{Emitter, WebviewWindow};

/// What the web title bar needs to know about the window chrome.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChromeInfo {
    /// `windows` | `macos` | `linux`.
    os: &'static str,
    /// Windows build (0 elsewhere).
    build: u32,
    /// The web title bar draws the window buttons (no system frame).
    custom: bool,
    /// The Mica backdrop is available (Windows 11).
    mica: bool,
}

fn info(os: &'static str, build: u32) -> ChromeInfo {
    let windows = os == "windows";
    ChromeInfo {
        os,
        build,
        custom: windows,
        mica: windows && bo_winchrome::has_mica(build),
    }
}

/// `windows` / `macos` / `linux` (other Unix desktops count as Linux).
fn os_name() -> &'static str {
    match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "macos",
        _ => "linux",
    }
}

#[cfg(windows)]
fn os_build() -> u32 {
    bo_winchrome::win32::os_build()
}

#[cfg(not(windows))]
fn os_build() -> u32 {
    0
}

#[tauri::command]
pub fn chrome_info() -> ChromeInfo {
    info(os_name(), os_build())
}

/// Event to the web title bar with the native maximise button's hover / press state.
pub const MAX_BUTTON_EVENT: &str = "bo-chrome:max-button";

/// A rectangle in CSS pixels of the web view.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct CssRectIn {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// Backdrop requested by the web UI (it follows the app theme; `none` for high contrast and
/// reduced transparency).
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Backdrop {
    MicaLight,
    MicaDark,
    None,
}

/// Applies the backdrop; `true` when Mica is now behind the web view.
#[tauri::command]
pub fn chrome_set_backdrop(window: WebviewWindow, backdrop: Backdrop) -> bool {
    use tauri::utils::config::WindowEffectsConfig;
    use tauri::window::Effect;
    let effect = match backdrop {
        Backdrop::MicaLight => Effect::MicaLight,
        Backdrop::MicaDark => Effect::MicaDark,
        Backdrop::None => {
            let _ = window.set_effects(None);
            return false;
        }
    };
    if !chrome_info().mica {
        return false;
    }
    let theme = if effect == Effect::MicaDark {
        tauri::Theme::Dark
    } else {
        tauri::Theme::Light
    };
    let _ = window.set_theme(Some(theme));
    window
        .set_effects(WindowEffectsConfig {
            effects: vec![effect],
            ..WindowEffectsConfig::default()
        })
        .is_ok()
}

#[derive(Clone, Serialize)]
struct MaxButtonState {
    hover: bool,
    pressed: bool,
}

/// Places the snap-layouts overlay over the web maximise button (`rect` in CSS pixels, `scale`
/// = `devicePixelRatio`); `None` removes it. No-op off Windows.
#[tauri::command]
pub fn chrome_set_max_button(
    window: WebviewWindow,
    rect: Option<CssRectIn>,
    scale: f64,
) -> Result<(), String> {
    #[cfg(windows)]
    {
        let parent = window.hwnd().map_err(|e| e.to_string())?.0 as isize;
        let target = window.clone();
        window
            .run_on_main_thread(move || {
                let client = bo_winchrome::win32::client_size(parent).unwrap_or((0, 0));
                let phys = rect.and_then(|r| {
                    bo_winchrome::to_physical(
                        bo_winchrome::CssRect {
                            x: r.x,
                            y: r.y,
                            width: r.width,
                            height: r.height,
                        },
                        scale,
                        client,
                    )
                });
                let _ = bo_winchrome::win32::set_maximize_button(parent, phys, move |s| {
                    let _ = target.emit_to(
                        target.label(),
                        MAX_BUTTON_EVENT,
                        MaxButtonState {
                            hover: s.hover,
                            pressed: s.pressed,
                        },
                    );
                });
            })
            .map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = (window, rect, scale);
        Ok(())
    }
}

/// Shows the system menu at a point of the web view (CSS pixels; right-click on the caption,
/// Alt+Space).
#[tauri::command]
pub fn chrome_system_menu(window: WebviewWindow, x: f64, y: f64, scale: f64) -> Result<(), String> {
    #[cfg(windows)]
    {
        let parent = window.hwnd().map_err(|e| e.to_string())?.0 as isize;
        let s = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let (px, py) = ((x * s).round() as i32, (y * s).round() as i32);
        window
            .run_on_main_thread(move || bo_winchrome::win32::show_system_menu(parent, px, py))
            .map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        let _ = (window, x, y, scale);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_chrome_and_mica_on_windows_only() {
        assert_eq!(
            info("windows", 22631),
            ChromeInfo {
                os: "windows",
                build: 22631,
                custom: true,
                mica: true
            }
        );
        let w10 = info("windows", 19045);
        assert!(w10.custom && !w10.mica);
        let mac = info("macos", 0);
        assert!(!mac.custom && !mac.mica);
    }

    #[test]
    fn backdrop_names_match_the_web() {
        let b: Backdrop = serde_json::from_str("\"micaDark\"").unwrap_or(Backdrop::None);
        assert_eq!(b, Backdrop::MicaDark);
        let b: Backdrop = serde_json::from_str("\"micaLight\"").unwrap_or(Backdrop::None);
        assert_eq!(b, Backdrop::MicaLight);
    }
}
