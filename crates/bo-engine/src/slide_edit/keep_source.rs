//! "Keep source formatting" for pasted shapes (CORE-PASTE-001): theme colours
//! (§20.1.2.3.29 schemeClr, mapped by §19.3.1.6 clrMap) and theme fonts
//! (§20.1.4.1.16 `+mj-lt` / `+mn-lt` … references of a:latin / a:ea / a:cs) are
//! replaced by the values they had on the source slide, so the shapes look the
//! same under another theme. The default ("use destination theme") keeps the
//! references, which then resolve against the destination's theme.
//!
//! What the shapes inherit from the source layout / master (§19.3.1.36 placeholder
//! inheritance: spPr fill and line; §19.3.1.51 txStyles, §21.1.2.4.12 lstStyle and the
//! §20.1.4.2.10 fontRef of p:style for text) is written onto them first
//! ([`pin_inherited`]), so the pasted copy does not pick up the destination master's look.

use bo_drawingml::color::{resolve, ColorContext};
use bo_model::slide::{Color, Presentation, RunProps, Shape, ShapeNode, TextRun};
use bo_pptx::inherit::{chain, clr_map_of, effective_props, theme_of, Level, TextStyleStack};
use serde_json::Value;

/// Theme values of the slide shapes were copied from.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SourceLook {
    /// Scheme colour name (`accent1`, `tx1`, …) → `0xRRGGBB`.
    pub colors: Vec<(String, u32)>,
    /// Theme font reference (`+mn-lt`, …) → typeface.
    pub fonts: Vec<(String, String)>,
    /// The copied shapes (same order) with their inherited look written on them.
    pub inherited: Vec<ShapeNode>,
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
    SourceLook {
        colors,
        fonts,
        inherited: Vec::new(),
    }
}

/// `n` (a shape of `slide`) with what it inherits from the slide's layout and master
/// written as its own properties: fill and line of placeholders, and the text size,
/// colour, fonts, bold / italic, alignment, indents, spacing and bullets of its runs and
/// paragraphs. Properties the shape sets itself are kept.
pub fn pin_inherited(pres: &Presentation, slide: usize, n: &ShapeNode) -> ShapeNode {
    let mut n = n.clone();
    pin_node(pres, slide, &mut n);
    n
}

fn pin_node(pres: &Presentation, slide: usize, n: &mut ShapeNode) {
    match n {
        ShapeNode::Shape(s) => pin_shape(pres, slide, s),
        ShapeNode::Group(g) => {
            for c in &mut g.children {
                pin_node(pres, slide, c);
            }
        }
        _ => {}
    }
}

fn pin_shape(pres: &Presentation, slide: usize, s: &mut Shape) {
    let level = Level::Slide(slide);
    let original = s.clone();
    let chain = chain(pres, level, &original);
    if chain.len() > 1 {
        let eff = effective_props(&chain);
        if s.props.fill.is_none() {
            s.props.fill = eff.fill;
        }
        if s.props.line.is_none() {
            s.props.line = eff.line;
        }
    }
    let Some(text) = s.text.as_mut() else {
        return;
    };
    let stack = TextStyleStack::new(pres, level, &chain);
    for p in &mut text.paragraphs {
        let ppr = stack.paragraph(p);
        for r in &mut p.runs {
            let props = match r {
                TextRun::Run { props, .. }
                | TextRun::Break { props }
                | TextRun::Field { props, .. } => props,
                TextRun::Other(_) => continue,
            };
            let eff = TextStyleStack::run(&ppr, props);
            pin_run(props, &eff);
        }
        if let Some(end) = p.end_props.as_mut() {
            let eff = TextStyleStack::run(&ppr, end);
            pin_run(end, &eff);
        }
        let own = p.props.get_or_insert_with(Default::default);
        macro_rules! pin {
            ($($f:ident),*) => { $( if own.$f.is_none() { own.$f.clone_from(&ppr.$f); } )* };
        }
        pin!(algn, mar_l, mar_r, indent, ln_spc, spc_bef, spc_aft, bullet, bu_clr, bu_sz, bu_font);
    }
}

fn pin_run(r: &mut RunProps, eff: &RunProps) {
    macro_rules! pin {
        ($($f:ident),*) => { $( if r.$f.is_none() { r.$f.clone_from(&eff.$f); } )* };
    }
    pin!(sz, b, i, u, cap, fill, latin, ea, cs);
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
            inherited: Vec::new(),
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
