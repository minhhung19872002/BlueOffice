//! OS shell integration for the desktop apps (prompt §6 Phase 6 "Desktop",
//! §6A.3): deep links (`bluedoc://` · `bluesheet://` · `blueslide://`),
//! launch-argument classification for file association / second instances,
//! and multi-window labels. Pure logic lives here so it is unit-testable
//! without a running Tauri app.

use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};

/// Schemes registered by the three desktop apps (ADR-0006).
pub const SCHEMES: &[(&str, &str)] = &[
    ("bluedoc", "doc"),
    ("bluesheet", "sheet"),
    ("blueslide", "slide"),
];

/// A parsed `blue*://open?host=…&file=…&token=…` link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeepLink {
    /// App id: `doc` / `sheet` / `slide`.
    pub app: String,
    /// Storage host id, e.g. `bluedrive`.
    pub host: String,
    pub file_id: String,
    /// Short-lived host access token (never logged).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

/// What the OS asked us to open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LaunchRequest {
    File {
        path: String,
    },
    Link(DeepLink),
    /// Sign-in redirect `blue*://auth?code=…&state=…` (OAuth 2.0 + PKCE, RFC 8252 §7.1
    /// private-use URI scheme), handed to the web view unchanged.
    Auth {
        url: String,
    },
}

/// Recognises a sign-in redirect to one of our schemes (`bluedoc://auth?…`).
pub fn parse_auth_callback(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    let scheme = scheme.to_ascii_lowercase();
    if !SCHEMES.iter().any(|(s, _)| *s == scheme) || url.len() > 8192 {
        return None;
    }
    let action = rest.split(['?', '#']).next().unwrap_or("");
    // `auth`: host sign-in (BlueDrive); `auth/oidc`: BlueOffice user sign-in.
    matches!(action.trim_end_matches('/'), "auth" | "auth/oidc").then(|| url.to_owned())
}

/// Opens only web pages in the system browser (sign-in); anything else is refused.
pub fn external_url(url: &str) -> Result<&str, String> {
    let lower = url.to_ascii_lowercase();
    let ok = (lower.starts_with("https://") || lower.starts_with("http://"))
        && url.len() <= 8192
        && !url
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '"');
    if ok {
        Ok(url)
    } else {
        Err("only http(s) URLs can be opened".into())
    }
}

/// File name of an offline-cache entry: the key's bytes in hex (never a path).
pub fn cache_file_name(key: &str) -> Result<String, String> {
    // 120 bytes → 244 characters: within the 255-character file name limit.
    if key.is_empty() || key.len() > 120 {
        return Err("invalid cache key".into());
    }
    let mut s = String::with_capacity(key.len() * 2 + 4);
    for b in key.bytes() {
        s.push_str(&format!("{b:02x}"));
    }
    s.push_str(".bin");
    Ok(s)
}

/// Parses a deep link; rejects unknown schemes/actions and missing ids.
pub fn parse_deep_link(url: &str) -> Result<DeepLink, String> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| "not a URL".to_owned())?;
    let scheme = scheme.to_ascii_lowercase();
    let app = SCHEMES
        .iter()
        .find(|(s, _)| *s == scheme)
        .map(|(_, a)| (*a).to_owned())
        .ok_or_else(|| format!("unsupported scheme '{scheme}'"))?;
    let (action, query) = rest.split_once('?').unwrap_or((rest, ""));
    if action.trim_end_matches('/') != "open" {
        return Err(format!("unsupported action '{action}'"));
    }
    let mut host = None;
    let mut file_id = None;
    let mut token = None;
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let v = percent_decode(&v.replace('+', " "));
        match k {
            "host" => host = Some(v),
            "file" => file_id = Some(v),
            "token" => token = Some(v),
            _ => {}
        }
    }
    let host = host
        .filter(|h| is_safe_id(h))
        .ok_or("missing or invalid 'host'")?;
    let file_id = file_id
        .filter(|f| !f.is_empty() && f.len() <= 512)
        .ok_or("missing 'file'")?;
    Ok(DeepLink {
        app,
        host,
        file_id,
        token: token.filter(|t| !t.is_empty()),
    })
}

fn is_safe_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

/// Classifies process arguments (first instance or forwarded by a second
/// instance): existing files and deep links; flags and unknowns are ignored.
pub fn classify_args<I, S>(args: I) -> Vec<LaunchRequest>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter()
        .filter_map(|a| {
            let a = a.as_ref();
            // WebView automation drivers (msedgedriver) forward app arguments as
            // `--<arg>`; accept that form when it names an existing file.
            if let Some(path) = a.strip_prefix("--").filter(|p| Path::new(p).is_file()) {
                return Some(LaunchRequest::File {
                    path: path.to_owned(),
                });
            }
            // … and the same for deep links (only our schemes parse).
            if let Some(l) = a.strip_prefix("--").filter(|l| l.contains("://")) {
                if let Some(url) = parse_auth_callback(l) {
                    return Some(LaunchRequest::Auth { url });
                }
                if let Ok(link) = parse_deep_link(l) {
                    return Some(LaunchRequest::Link(link));
                }
            }
            if a.starts_with('-') {
                None
            } else if a.contains("://") {
                parse_auth_callback(a)
                    .map(|url| LaunchRequest::Auth { url })
                    .or_else(|| parse_deep_link(a).ok().map(LaunchRequest::Link))
            } else if Path::new(a).is_file() {
                Some(LaunchRequest::File { path: a.to_owned() })
            } else {
                None
            }
        })
        .collect()
}

static WINDOW_SEQ: AtomicU32 = AtomicU32::new(0);

/// Unique label for an additional document window (matches the `doc-*`
/// capability glob).
pub fn next_window_label() -> String {
    format!("doc-{}", WINDOW_SEQ.fetch_add(1, Ordering::Relaxed) + 1)
}

/// View › Arrange All: `n` windows tiled over the area `(x, y, w, h)` (logical pixels) —
/// columns side by side up to three windows, then a grid filled row by row. Returns the
/// position and size of each window, in order.
pub fn tile_windows(n: usize, area: (f64, f64, f64, f64)) -> Vec<(f64, f64, f64, f64)> {
    if n == 0 {
        return Vec::new();
    }
    let (x, y, w, h) = area;
    let cols = if n <= 3 {
        n
    } else {
        (1..=n).find(|c| c * c >= n).unwrap_or(n)
    };
    let rows = n.div_ceil(cols);
    #[allow(clippy::cast_precision_loss)]
    let (cw, rh) = (w / cols as f64, h / rows as f64);
    (0..n)
        .map(|i| {
            #[allow(clippy::cast_precision_loss)]
            let (c, r) = ((i % cols) as f64, (i / cols) as f64);
            (x + c * cw, y + r * rh, cw, rh)
        })
        .collect()
}

/// Is `label` a document window (the main one or one opened by `open_window`)?
pub fn is_document_window(label: &str) -> bool {
    label == "main" || label.starts_with("doc-")
}

/// Only relative app URLs with a query may be opened in new windows.
pub fn window_url(query: &str) -> Result<String, String> {
    let q = query.trim_start_matches('?');
    if q.contains("://") || q.contains('#') || q.starts_with('/') {
        return Err("invalid window query".into());
    }
    Ok(format!("index.html?{q}"))
}

/// Decode `encodeURIComponent` output.
pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Some(v) = s
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
            {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Environment variable naming the BlueOffice server used for host files
/// opened by deep link (the web view stores a user setting as fallback).
pub const SERVER_ENV: &str = "BLUEOFFICE_SERVER";

/// Normalises a BlueOffice server URL: `http(s)://authority[/path]`, no
/// query/fragment, without the trailing slash. Anything else is ignored.
pub fn normalize_server_url(value: &str) -> Option<String> {
    let v = value.trim().trim_end_matches('/');
    let rest = v
        .strip_prefix("https://")
        .or_else(|| v.strip_prefix("http://"))?;
    let authority = rest.split('/').next().unwrap_or("");
    let ok = !authority.is_empty()
        && !v.contains(['?', '#', ' ', '@'])
        && authority
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'));
    ok.then(|| v.to_owned())
}

/// E2E hook: `BLUEOFFICE_E2E_SAVE_DIR=<dir>` makes "Save as" write into that
/// folder under the suggested name instead of opening the OS save dialog
/// (which WebDriver cannot drive).
pub const E2E_SAVE_DIR_ENV: &str = "BLUEOFFICE_E2E_SAVE_DIR";

/// Target of a dialog-less save: the file name part of `suggested` inside `dir`.
pub fn e2e_save_path(dir: &str, suggested: &str) -> Option<std::path::PathBuf> {
    let name = Path::new(suggested.rsplit(['/', '\\']).next().unwrap_or(""))
        .file_name()?
        .to_owned();
    (!dir.trim().is_empty() && name != "." && name != "..")
        .then(|| Path::new(dir.trim()).join(name))
}

/// `BLUEOFFICE_SYSTEM_FONTS=0` keeps documents on the bundled fonts only
/// (same rendering as the web app, used by the parity E2E).
pub const SYSTEM_FONTS_ENV: &str = "BLUEOFFICE_SYSTEM_FONTS";

/// Are installed OS fonts used (default yes)?
pub fn system_fonts_enabled(env: Option<&str>) -> bool {
    !matches!(
        env.map(|v| v.trim().to_ascii_lowercase()).as_deref(),
        Some("0" | "false" | "off" | "no")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e2e_saves_stay_inside_the_folder() {
        let p = e2e_save_path("C:\\out", "Báo cáo.docx").unwrap();
        assert_eq!(p, Path::new("C:\\out").join("Báo cáo.docx"));
        let p = e2e_save_path("/tmp/out", "../../etc/passwd").unwrap();
        assert_eq!(p, Path::new("/tmp/out").join("passwd"));
        assert!(e2e_save_path("/tmp/out", "..").is_none());
        assert!(e2e_save_path(" ", "a.docx").is_none());
    }

    #[test]
    fn tiles_windows_side_by_side_then_in_a_grid() {
        assert_eq!(tile_windows(0, (0.0, 0.0, 1200.0, 800.0)).len(), 0);
        assert_eq!(
            tile_windows(2, (10.0, 20.0, 1200.0, 800.0)),
            vec![(10.0, 20.0, 600.0, 800.0), (610.0, 20.0, 600.0, 800.0)]
        );
        let four = tile_windows(4, (0.0, 0.0, 1200.0, 800.0));
        assert_eq!(four[0], (0.0, 0.0, 600.0, 400.0));
        assert_eq!(four[3], (600.0, 400.0, 600.0, 400.0));
        // Five windows: three columns, two rows.
        let five = tile_windows(5, (0.0, 0.0, 1200.0, 800.0));
        assert_eq!(five[4], (400.0, 400.0, 400.0, 400.0));
        assert!(is_document_window("main") && is_document_window("doc-3"));
        assert!(!is_document_window("print-1"));
    }

    #[test]
    fn system_fonts_switch() {
        assert!(system_fonts_enabled(None));
        assert!(system_fonts_enabled(Some("1")));
        assert!(!system_fonts_enabled(Some("0")));
        assert!(!system_fonts_enabled(Some(" Off")));
    }

    #[test]
    fn normalizes_server_urls() {
        assert_eq!(
            normalize_server_url(" https://office.example.vn/ ").as_deref(),
            Some("https://office.example.vn")
        );
        assert_eq!(
            normalize_server_url("http://localhost:4583").as_deref(),
            Some("http://localhost:4583")
        );
        assert_eq!(
            normalize_server_url("https://x.vn/office").as_deref(),
            Some("https://x.vn/office")
        );
        assert!(normalize_server_url("ftp://x.vn").is_none());
        assert!(normalize_server_url("https://").is_none());
        assert!(normalize_server_url("https://user@x.vn").is_none());
        assert!(normalize_server_url("https://x.vn/?a=1").is_none());
        assert!(normalize_server_url("javascript:alert(1)").is_none());
    }

    #[test]
    fn parses_bluedrive_link() {
        let l = parse_deep_link("bluedoc://open?host=bluedrive&file=F%2042&token=abc").unwrap();
        assert_eq!(
            l,
            DeepLink {
                app: "doc".into(),
                host: "bluedrive".into(),
                file_id: "F 42".into(),
                token: Some("abc".into()),
            }
        );
        assert_eq!(
            parse_deep_link("BlueSheet://open/?file=x&host=h")
                .unwrap()
                .app,
            "sheet"
        );
    }

    #[test]
    fn rejects_bad_links() {
        assert!(parse_deep_link("http://open?host=a&file=b").is_err());
        assert!(parse_deep_link("bluedoc://delete?host=a&file=b").is_err());
        assert!(parse_deep_link("bluedoc://open?file=b").is_err());
        assert!(parse_deep_link("bluedoc://open?host=a/b&file=b").is_err());
        assert!(parse_deep_link("bluedoc://open?host=a").is_err());
    }

    #[test]
    fn token_is_not_serialized_when_absent() {
        let l = parse_deep_link("blueslide://open?host=h&file=1").unwrap();
        let json = serde_json::to_string(&LaunchRequest::Link(l)).unwrap();
        assert_eq!(
            json,
            r#"{"kind":"link","app":"slide","host":"h","fileId":"1"}"#
        );
    }

    #[test]
    fn classifies_args() {
        let manifest = env!("CARGO_MANIFEST_DIR").to_owned() + "/Cargo.toml";
        let dashed = format!("--{manifest}");
        let reqs = classify_args([
            "bluedoc.exe-not-a-file",
            "--flag",
            dashed.as_str(),
            "bluedoc://open?host=bluedrive&file=1",
            "--bluesheet://open?host=bluedrive&file=2&token=a.b-c_d",
            "--https://evil.example/open?host=a&file=b",
        ]);
        assert_eq!(reqs.len(), 3);
        // `--<file>` / `--<link>` (automation drivers) are understood as the file / link.
        assert!(matches!(&reqs[0], LaunchRequest::File { path } if path == &manifest));
        assert!(matches!(&reqs[1], LaunchRequest::Link(l) if l.file_id == "1"));
        assert!(matches!(&reqs[2], LaunchRequest::Link(l)
            if l.app == "sheet" && l.token.as_deref() == Some("a.b-c_d")));
    }

    #[test]
    fn recognises_sign_in_redirects() {
        let cb = "bluedoc://auth?code=a%2Bb&state=xyz";
        assert_eq!(parse_auth_callback(cb).as_deref(), Some(cb));
        assert!(parse_auth_callback("BlueSheet://auth/?code=1").is_some());
        // BlueOffice user sign-in (OIDC) of the desktop app.
        assert!(parse_auth_callback("bluedoc://auth/oidc?code=1&state=s").is_some());
        assert!(parse_auth_callback("bluedoc://auth/other?code=1").is_none());
        assert!(parse_auth_callback("bluedoc://open?host=a&file=b").is_none());
        assert!(parse_auth_callback("https://auth?code=1").is_none());
        let reqs = classify_args([cb, "--bluedoc://auth?code=2&state=s"]);
        assert!(matches!(&reqs[0], LaunchRequest::Auth { url } if url == cb));
        assert!(matches!(&reqs[1], LaunchRequest::Auth { url } if url.ends_with("code=2&state=s")));
    }

    #[test]
    fn opens_only_web_urls_externally() {
        assert!(external_url("https://sso.example.vn/auth?x=1&y=2").is_ok());
        assert!(external_url("http://localhost:5080/oauth/authorize").is_ok());
        assert!(external_url("file:///C:/Windows/system32/calc.exe").is_err());
        assert!(external_url("https://x.vn/\" & calc").is_err());
        assert!(external_url("javascript:alert(1)").is_err());
    }

    #[test]
    fn cache_keys_become_flat_file_names() {
        assert_eq!(cache_file_name("a/..").unwrap(), "612f2e2e.bin");
        assert!(cache_file_name("").is_err());
        assert!(cache_file_name(&"k".repeat(121)).is_err());
        assert!(cache_file_name(&"k".repeat(120)).unwrap().len() < 255);
    }

    #[test]
    fn window_labels_and_urls() {
        assert_ne!(next_window_label(), next_window_label());
        assert_eq!(window_url("?app=sheet").unwrap(), "index.html?app=sheet");
        assert!(window_url("https://evil").is_err());
        assert!(window_url("/etc").is_err());
    }

    #[test]
    fn decodes_uri_components() {
        assert_eq!(
            percent_decode("T%C3%A0i%20li%E1%BB%87u.docx"),
            "Tài liệu.docx"
        );
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("%zz%C3%A0"), "%zzà");
    }
}
