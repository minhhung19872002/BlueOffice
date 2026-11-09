//! "Keep source formatting" for pasted shapes (CORE-PASTE-001): theme colours
//! (§20.1.2.3.29 schemeClr, mapped by §19.3.1.6 clrMap) and theme fonts
//! (§20.1.4.1.16 `+mj-lt` / `+mn-lt` … references of a:latin / a:ea / a:cs) are
//! replaced by the values they had on the source slide, so the shapes look the
//! same under another theme. The default ("use destination theme") keeps the
//! references, which then resolve against the destination's theme.

use bo_drawingml::color::{resolve, ColorContext};
use bo_model::slide::{Color, Presentation, ShapeNode};
use bo_pptx::inherit::{clr_map_of, theme_of, Level};
use serde_json::Value;

/// Theme values of the slide shapes were copied from.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SourceLook {
    /// Scheme colour name (`accent1`, `tx1`, …) → `0xRRGGBB`.
    pub colors: Vec<(String, u32)>,
    /// Theme font reference (`+mn-lt`, …) → typeface.
    pub fonts: Vec<(String, String)>,
}

/// Scheme colour names a shape may use (§20.1.10.54 ST_SchemeColorVal, without `phClr`).
const SCHEME_NAMES: [&str; 16] = [
    "bg1", "tx1", "bg2", "tx2", "dk1", "lt1", "dk2", "lt2", "accent1", "accent2", "accent3",
    "accent4", "accent5", "accent6", "hlink", "folHlink",
];

/// Theme colours and fonts as they resolve on `slide`.
pub fn source_look(pres: &Presentation, slide: usize) -> SourceLook {
    let level = Level::Slide(slide);
    let theme = theme_of(pres, level);
    let map = clr_map_of(pres, level);
    let ctx = ColorContext {
        theme,
        clr_map: &map,
        ph_clr: None,
    };
    let colors = SCHEME_NAMES
        .iter()
        .map(|n| ((*n).to_owned(), resolve(&Color::scheme(n), &ctx).to_hex()))
        .collect();
    let mut fonts = Vec::new();
    if let Some(t) = theme {
        let f = &t.fonts;
        for (prefix, c) in [("+mj", &f.major), ("+mn", &f.minor)] {
            for (script, tf) in [("lt", &c.latin), ("ea", &c.ea), ("cs", &c.cs)] {
                if !tf.typeface.is_empty() {
                    fonts.push((format!("{prefix}-{script}"), tf.typeface.clone()));
                }
            }
        }
    }
    SourceLook { colors, fonts }
}

/// `n` with its theme colour and font references replaced by the source values.
/// Unchanged when the shape cannot be converted.
pub fn apply_look(n: &ShapeNode, look: &SourceLook) -> ShapeNode {
    let Ok(mut v) = serde_json::to_value(n) else {
        return n.clone();
    };
    walk(&mut v, look);
    serde_json::from_value(v).unwrap_or_else(|_| n.clone())
}

fn walk(v: &mut Value, look: &SourceLook) {
    match v {
        Value::Object(map) => {
            // A modelled colour: `{"kind": {"Scheme": name}, "mods": […]}` (transforms kept).
            if let Some(Value::Object(kind)) = map.get_mut("kind") {
                let rgb = match kind.get("Scheme") {
                    Some(Value::String(name)) => {
                        look.colors.iter().find(|(n, _)| n == name).map(|(_, c)| *c)
                    }
                    _ => None,
                };
                if let Some(rgb) = rgb {
                    kind.clear();
                    kind.insert("Srgb".into(), Value::from(rgb));
                }
            }
            // A theme font reference: `{"typeface": "+mn-lt", …}`.
            if let Some(Value::String(face)) = map.get_mut("typeface") {
                if let Some((_, f)) = look.fonts.iter().find(|(r, _)| r == face) {
                    face.clone_from(f);
                }
            }
            for x in map.values_mut() {
                walk(x, look);
            }
        }
        Value::Array(items) => {
            for x in items {
                walk(x, look);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheme_colours_and_theme_fonts_become_literal() {
        let look = SourceLook {
            colors: vec![("accent1".into(), 0x00_44_72_C4), ("tx1".into(), 0)],
            fonts: vec![("+mn-lt".into(), "Carlito".into())],
        };
        let mut v = serde_json::json!({
            "fill": {"kind": {"Scheme": "accent1"}, "mods": [{"name": "lumMod", "val": 75000}]},
            "ph": {"kind": {"Scheme": "phClr"}, "mods": []},
            "runs": [{"latin": {"typeface": "+mn-lt", "attrs": []}},
                     {"latin": {"typeface": "Arial", "attrs": []}}]
        });
        walk(&mut v, &look);
        assert_eq!(
            v["fill"]["kind"],
            serde_json::json!({"Srgb": 0x00_44_72_C4})
        );
        assert_eq!(v["fill"]["mods"][0]["val"], 75000);
        // phClr belongs to the style reference: unchanged.
        assert_eq!(v["ph"]["kind"]["Scheme"], "phClr");
        assert_eq!(v["runs"][0]["latin"]["typeface"], "Carlito");
        assert_eq!(v["runs"][1]["latin"]["typeface"], "Arial");
    }
}
