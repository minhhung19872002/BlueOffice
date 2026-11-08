//! Auto-update (prompt §6 Phase 6 "Desktop: auto-update"; Tauri updater
//! plugin, feature `updater`). Endpoint, public key and release manifest are
//! described in `docs/deploy/desktop-update.md`. The pure decisions (which
//! endpoints, whether updating is configured, what the UI is told) live here
//! so they are unit-tested without a network or a running app.
// Without the `updater` feature only `Disabled` is reported.
#![cfg_attr(not(feature = "updater"), allow(dead_code))]

use serde::Serialize;

/// Comma-separated update endpoints overriding `plugins.updater.endpoints`
/// (on-prem update server). `{{target}}`, `{{arch}}`, `{{current_version}}` are expanded.
pub const URL_ENV: &str = "BLUEOFFICE_UPDATE_URL";

/// Public key placeholder shipped in `tauri.conf.json`: the release pipeline
/// replaces it with the minisign public key of the signing key.
pub const PUBKEY_PLACEHOLDER: &str = "REPLACE_WITH_BLUESTAR_UPDATER_PUBLIC_KEY";

/// What the "Check for updates" command reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum UpdateStatus {
    /// Built without the `updater` feature (dev builds).
    #[cfg_attr(feature = "updater", allow(dead_code))]
    Disabled,
    /// The release config still has the placeholder key or no endpoint.
    NotConfigured,
    UpToDate {
        current: String,
    },
    Available {
        current: String,
        version: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        notes: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        date: Option<String>,
    },
    /// Endpoint unreachable, bad manifest, …
    Failed {
        message: String,
    },
}

/// Endpoints from `BLUEOFFICE_UPDATE_URL` (when set and valid), else none
/// (the plugin then uses `plugins.updater.endpoints`).
pub fn env_endpoints(value: Option<&str>) -> Result<Vec<String>, String> {
    let Some(v) = value.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(Vec::new());
    };
    v.split(',')
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .map(|u| {
            let lower = u.to_ascii_lowercase();
            if (lower.starts_with("https://") || lower.starts_with("http://"))
                && u.len() <= 2048
                && !u.chars().any(|c| c.is_whitespace() || c.is_control())
            {
                Ok(u.to_owned())
            } else {
                Err(format!("{URL_ENV}: not an http(s) URL: {u}"))
            }
        })
        .collect()
}

/// Can updates be checked: a real public key and at least one endpoint.
pub fn configured(pubkey: &str, config_endpoints: usize, env_endpoints: usize) -> bool {
    let key = pubkey.trim();
    !key.is_empty() && key != PUBKEY_PLACEHOLDER && config_endpoints + env_endpoints > 0
}

/// Status of a finished check (`None` = the plugin found no newer release).
pub fn status_of(
    current: &str,
    found: Option<(String, Option<String>, Option<String>)>,
) -> UpdateStatus {
    match found {
        None => UpdateStatus::UpToDate {
            current: current.to_owned(),
        },
        Some((version, notes, date)) => UpdateStatus::Available {
            current: current.to_owned(),
            version,
            notes: notes.filter(|n| !n.trim().is_empty()),
            date,
        },
    }
}

/// Reads `plugins.updater` of the app config: (pubkey, endpoint count).
pub fn plugin_config(plugins: &serde_json::Value) -> (String, usize) {
    let u = &plugins["updater"];
    (
        u["pubkey"].as_str().unwrap_or_default().to_owned(),
        u["endpoints"].as_array().map_or(0, Vec::len),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn env_endpoints_override_the_config() {
        assert_eq!(env_endpoints(None), Ok(vec![]));
        assert_eq!(env_endpoints(Some("  ")), Ok(vec![]));
        assert_eq!(
            env_endpoints(Some(
                "https://u.example/a.json, https://u2.example/{{target}}"
            )),
            Ok(vec![
                "https://u.example/a.json".to_owned(),
                "https://u2.example/{{target}}".to_owned()
            ])
        );
        assert!(env_endpoints(Some("file:///etc/passwd")).is_err());
        assert!(env_endpoints(Some("https://a b")).is_err());
    }

    #[test]
    fn placeholder_key_or_no_endpoint_is_not_configured() {
        assert!(!configured(PUBKEY_PLACEHOLDER, 1, 0));
        assert!(!configured("", 1, 0));
        assert!(!configured("dW50cnVzdGVk", 0, 0));
        assert!(configured("dW50cnVzdGVk", 0, 1));
        assert!(configured("dW50cnVzdGVk", 2, 0));
    }

    #[test]
    fn statuses_serialize_for_the_ui() {
        let s = status_of("0.1.0", None);
        assert_eq!(
            serde_json::to_value(&s).unwrap(),
            json!({"state":"upToDate","current":"0.1.0"})
        );
        let s = status_of(
            "0.1.0",
            Some(("0.2.0".into(), Some(" ".into()), Some("2026-11-01".into()))),
        );
        assert_eq!(
            serde_json::to_value(&s).unwrap(),
            json!({"state":"available","current":"0.1.0","version":"0.2.0","date":"2026-11-01"})
        );
        assert_eq!(
            serde_json::to_value(UpdateStatus::NotConfigured).unwrap(),
            json!({"state":"notConfigured"})
        );
        assert_eq!(
            serde_json::to_value(UpdateStatus::Failed {
                message: "x".into()
            })
            .unwrap(),
            json!({"state":"failed","message":"x"})
        );
    }

    #[test]
    fn shipped_config_has_a_documented_updater_section() {
        for conf in [
            include_str!("../tauri.conf.json"),
            include_str!("../tauri.bluesheet.conf.json"),
            include_str!("../tauri.blueslide.conf.json"),
        ] {
            let v: serde_json::Value = serde_json::from_str(conf).unwrap();
            if v["plugins"]["updater"].is_null() {
                continue; // Overlay without its own endpoint.
            }
            let (key, n) = plugin_config(&v["plugins"]);
            assert_eq!(key, PUBKEY_PLACEHOLDER);
            assert!(n >= 1);
            for e in v["plugins"]["updater"]["endpoints"].as_array().unwrap() {
                let e = e.as_str().unwrap();
                assert!(e.starts_with("https://"), "{e}");
                assert!(e.contains("{{target}}") && e.contains("{{current_version}}"));
            }
        }
        let main: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert!(!main["plugins"]["updater"].is_null());
    }
}
