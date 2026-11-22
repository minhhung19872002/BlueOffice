//! Interface strings shared with the React interface (`packages/i18n/src/locales/*.json`).
use std::collections::HashMap;

const VI: &str = include_str!("../../../packages/i18n/src/locales/vi-VN.json");
const EN: &str = include_str!("../../../packages/i18n/src/locales/en-US.json");

/// Flattened `a.b.c` → text table of one locale.
pub struct Strings(HashMap<String, String>);

fn flatten(prefix: &str, v: &serde_json::Value, out: &mut HashMap<String, String>) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                let key = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(&key, v, out);
            }
        }
        serde_json::Value::String(s) => {
            out.insert(prefix.to_owned(), s.clone());
        }
        _ => {}
    }
}

impl Strings {
    /// Strings of `locale` (`vi-VN` or `en-US`; anything else falls back to Vietnamese).
    pub fn load(locale: &str) -> Self {
        let src = if locale.starts_with("en") { EN } else { VI };
        let mut map = HashMap::new();
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(src) {
            flatten("", &v, &mut map);
        }
        Self(map)
    }

    /// Text of `key` with `{{name}}` placeholders replaced by `args`.
    pub fn fmt(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut out = self.get(key).to_owned();
        for (name, value) in args {
            out = out.replace(&format!("{{{{{name}}}}}"), value);
        }
        out
    }

    /// Text of `key`, or the key itself when it is missing.
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.0.get(key).map_or(key, String::as_str)
    }

    /// Keys starting with `prefix` and their texts (command search).
    pub(crate) fn with_prefix<'a>(
        &'a self,
        prefix: &'a str,
    ) -> impl Iterator<Item = (&'a str, &'a str)> + 'a {
        self.0
            .iter()
            .filter(move |(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Set `key` to `value` (a window naming its own commands from existing strings).
    pub(crate) fn set(&mut self, key: &str, value: &str) {
        self.0.insert(key.to_owned(), value.to_owned());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn shared_locale_keys_resolve() {
        let s = super::Strings::load("vi-VN");
        assert_eq!(s.get("ribbon.tabs.home"), "Trang đầu");
        assert_eq!(s.get("commands.format.bold"), "Đậm");
        assert_eq!(s.get("no.such.key"), "no.such.key");
        assert_eq!(
            s.fmt("status.page", &[("current", "2"), ("total", "5")]),
            "Trang 2/5"
        );
        assert_eq!(
            super::Strings::load("en-US").get("ribbon.tabs.home"),
            "Home"
        );
    }
}
