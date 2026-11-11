//! WebDriver automation of the desktop E2E (`webdriver` feature, never in shipped builds).
//!
//! msedgedriver drives a WebView2 app through the `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`
//! (remote debugging port, …) and `WEBVIEW2_USER_DATA_FOLDER` environment variables. WebView2
//! ignores those variables in an elevated process (CI runners run as an elevated administrator),
//! so a build with the `webdriver` feature passes them to its web views through the WebView2 API
//! itself while `tauri-driver` is driving it (`TAURI_WEBVIEW_AUTOMATION=true`). Its main window
//! is then created at startup by [`create_main`] (the E2E build marks it `"create": false` in
//! `tauri.webdriver.conf.json`), so every window gets the same arguments and profile folder.

use tauri::{Runtime, WebviewWindowBuilder};

/// What wry passes by default: kept when other arguments are given.
#[cfg(feature = "webdriver")]
const WRY_DEFAULT_ARGS: &str = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";

/// Browser arguments and profile folder asked for by the WebDriver, if any.
#[cfg(feature = "webdriver")]
fn requested(
    automation: Option<&str>,
    args: Option<&str>,
    folder: Option<&str>,
) -> Option<(String, Option<std::path::PathBuf>)> {
    if automation != Some("true") {
        return None;
    }
    let args = args.map(str::trim).filter(|a| !a.is_empty())?;
    let folder = folder
        .filter(|f| !f.is_empty())
        .map(std::path::PathBuf::from);
    Some((format!("{WRY_DEFAULT_ARGS} {args}"), folder))
}

/// `b` with the WebDriver's browser arguments and profile folder when it is driving the app.
#[allow(clippy::needless_pass_by_value)]
pub fn apply<R: Runtime, M: tauri::Manager<R>>(
    b: WebviewWindowBuilder<'_, R, M>,
) -> WebviewWindowBuilder<'_, R, M> {
    #[cfg(feature = "webdriver")]
    {
        let var = |k: &str| std::env::var(k).ok();
        if let Some((args, folder)) = requested(
            var("TAURI_WEBVIEW_AUTOMATION").as_deref(),
            var("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").as_deref(),
            var("WEBVIEW2_USER_DATA_FOLDER").as_deref(),
        ) {
            let b = b.additional_browser_args(&args);
            return match folder {
                Some(f) => b.data_directory(f),
                None => b,
            };
        }
    }
    b
}

/// Creates the configured windows marked `"create": false` (the E2E build's main window).
#[cfg(feature = "webdriver")]
pub fn create_main<R: Runtime>(app: &tauri::App<R>) -> tauri::Result<()> {
    let windows: Vec<_> = app
        .config()
        .app
        .windows
        .iter()
        .filter(|w| !w.create)
        .cloned()
        .collect();
    for w in windows {
        apply(WebviewWindowBuilder::from_config(app.handle(), &w)?).build()?;
    }
    Ok(())
}

#[cfg(all(test, feature = "webdriver"))]
mod tests {
    use super::requested;

    #[test]
    fn only_while_driven_and_keeps_the_defaults() {
        let args = Some("--remote-debugging-port=0");
        assert_eq!(requested(None, args, Some("C:\\p")), None);
        assert_eq!(requested(Some("true"), None, Some("C:\\p")), None);
        let (a, f) = requested(Some("true"), args, Some("C:\\p")).unwrap_or_default();
        assert!(a.starts_with("--disable-features=msWebOOUI"));
        assert!(a.ends_with(" --remote-debugging-port=0"));
        assert_eq!(f, Some(std::path::PathBuf::from("C:\\p")));
        assert_eq!(
            requested(Some("true"), args, Some("")).and_then(|r| r.1),
            None
        );
    }
}
