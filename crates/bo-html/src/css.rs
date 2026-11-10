//! The CSS an importer needs: declaration lists (`style="…"`), simple style sheets
//! (`<style>` rules with type, class and id selectors) and value parsing (colours,
//! lengths, font names) — CSS Syntax Module Level 3 §5 and CSS 2.2 §4.3, reduced.

use crate::dom::Element;
use bo_model::Color;

/// Declarations of one element: property (lower case) → value, later ones winning.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Decls(pub Vec<(String, String)>);

impl Decls {
    /// Value of `prop`, the last declaration winning.
    pub fn get(&self, prop: &str) -> Option<&str> {
        self.0
            .iter()
            .rev()
            .find(|(k, _)| k == prop)
            .map(|(_, v)| v.as_str())
    }

    /// Append the declarations of `other` (they win over the present ones).
    pub fn extend(&mut self, other: &Decls) {
        self.0.extend(other.0.iter().cloned());
    }

    /// No declaration.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Parse a declaration list (`a: b; c: d`). `!important` is dropped; comments are skipped.
pub fn parse_decls(s: &str) -> Decls {
    let s = strip_comments(s);
    let mut out = Vec::new();
    for item in split_top(&s, ';') {
        let Some((k, v)) = item.split_once(':') else {
            continue;
        };
        let k = k.trim().to_ascii_lowercase();
        let v = v.trim();
        let v = v
            .strip_suffix("!important")
            .or_else(|| v.strip_suffix("! important"))
            .unwrap_or(v)
            .trim();
        if !k.is_empty() && !v.is_empty() && out.len() < 256 {
            out.push((k, v.to_owned()));
        }
    }
    Decls(out)
}

fn strip_comments(s: &str) -> String {
    if !s.contains("/*") {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(a) = rest.find("/*") {
        out.push_str(&rest[..a]);
        rest = match rest[a + 2..].find("*/") {
            Some(b) => &rest[a + 2 + b + 2..],
            None => "",
        };
    }
    out.push_str(rest);
    out
}

/// Split at `sep` outside quotes and parentheses.
fn split_top(s: &str, sep: char) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0u32;
    let mut quote: Option<char> = None;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => quote = Some(c),
                '(' => depth += 1,
                ')' => depth = depth.saturating_sub(1),
                c if c == sep && depth == 0 => {
                    out.push(&s[start..i]);
                    start = i + c.len_utf8();
                }
                _ => {}
            },
        }
    }
    out.push(&s[start..]);
    out
}

/// A simple selector: optional type, classes and id (`p`, `.a`, `td.x.y`, `#id`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct Selector {
    tag: Option<String>,
    classes: Vec<String>,
    id: Option<String>,
}

impl Selector {
    fn parse(s: &str) -> Option<Selector> {
        let s = s.trim();
        if s.is_empty()
            || s.contains(|c: char| {
                c.is_whitespace() || matches!(c, '>' | '+' | '~' | '[' | ':' | '*' | ',')
            })
        {
            return None;
        }
        let mut sel = Selector::default();
        let mut rest = s;
        let first = rest.find(['.', '#']).unwrap_or(rest.len());
        if first > 0 {
            sel.tag = Some(rest[..first].to_ascii_lowercase());
        }
        rest = &rest[first..];
        while !rest.is_empty() {
            let kind = rest.chars().next()?;
            let body = &rest[1..];
            let end = body.find(['.', '#']).unwrap_or(body.len());
            if end == 0 {
                return None;
            }
            match kind {
                '.' => sel.classes.push(body[..end].to_owned()),
                _ => sel.id = Some(body[..end].to_owned()),
            }
            rest = &body[end..];
        }
        Some(sel)
    }

    fn matches(&self, e: &Element) -> bool {
        self.tag.as_deref().is_none_or(|t| t == e.name)
            && self.id.as_deref().is_none_or(|i| e.attr("id") == Some(i))
            && self.classes.iter().all(|c| e.has_class(c))
    }

    /// Specificity: ids, classes, type (CSS Selectors §17).
    fn specificity(&self) -> u32 {
        u32::from(self.id.is_some()) * 10_000
            + self.classes.len().min(99) as u32 * 100
            + u32::from(self.tag.is_some())
    }
}

/// Rules of the `<style>` elements of a document.
#[derive(Debug, Clone, Default)]
pub struct StyleSheet {
    rules: Vec<(Selector, Decls)>,
    /// Rules by their most selective part (`#id`, else `.class`, else type), so an
    /// element only meets the rules that can match it.
    index: std::collections::HashMap<String, Vec<usize>>,
}

/// Index key of a selector: its id, else its first class, else its type.
fn rule_key(s: &Selector) -> String {
    if let Some(id) = &s.id {
        format!("#{id}")
    } else if let Some(c) = s.classes.first() {
        format!(".{c}")
    } else {
        s.tag.clone().unwrap_or_default()
    }
}

/// Most rules kept from the style sheets of one document.
const MAX_RULES: usize = 4096;

impl StyleSheet {
    /// Add the rules of a style sheet text. At-rules and selectors with
    /// combinators, attributes or pseudo-classes are skipped.
    pub fn add(&mut self, css: &str) {
        let css = strip_comments(css).replace("<!--", "").replace("-->", "");
        let mut rest = css.as_str();
        while let Some(open) = rest.find('{') {
            let prelude = rest[..open].trim();
            // The matching close brace (at-rules nest blocks).
            let mut depth = 0usize;
            let mut close = None;
            for (i, c) in rest[open..].char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            close = Some(open + i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let Some(close) = close else { break };
            if !prelude.starts_with('@') {
                let decls = parse_decls(&rest[open + 1..close]);
                if !decls.is_empty() {
                    for sel in prelude.split(',') {
                        if let Some(sel) = Selector::parse(sel) {
                            if self.rules.len() < MAX_RULES {
                                self.index
                                    .entry(rule_key(&sel))
                                    .or_default()
                                    .push(self.rules.len());
                                self.rules.push((sel, decls.clone()));
                            }
                        }
                    }
                }
            }
            rest = &rest[close + 1..];
        }
    }

    /// Declarations that apply to `e`: matching rules by specificity then source
    /// order, then the element's own `style` attribute.
    pub fn computed(&self, e: &Element) -> Decls {
        let mut hits: Vec<(u32, usize)> = Vec::new();
        if !self.rules.is_empty() {
            let mut keys: Vec<String> = vec![e.name.clone()];
            if let Some(id) = e.attr("id") {
                keys.push(format!("#{id}"));
            }
            if let Some(classes) = e.attr("class") {
                keys.extend(
                    classes
                        .split_ascii_whitespace()
                        .take(32)
                        .map(|c| format!(".{c}")),
                );
            }
            for key in keys {
                for &i in self.index.get(&key).map_or(&[][..], Vec::as_slice) {
                    let s = &self.rules[i].0;
                    if s.matches(e) {
                        hits.push((s.specificity(), i));
                    }
                }
            }
        }
        hits.sort_unstable();
        hits.dedup();
        let mut out = Decls::default();
        for (_, i) in hits {
            out.extend(&self.rules[i].1);
        }
        if let Some(style) = e.attr("style") {
            out.extend(&parse_decls(style));
        }
        out
    }
}

/// CSS colour keywords (CSS Color Level 3 §4.1 basic keywords and common extended ones).
const COLORS: &[(&str, u32)] = &[
    ("black", 0x00_0000),
    ("silver", 0xC0_C0C0),
    ("gray", 0x80_8080),
    ("grey", 0x80_8080),
    ("white", 0xFF_FFFF),
    ("maroon", 0x80_0000),
    ("red", 0xFF_0000),
    ("purple", 0x80_0080),
    ("fuchsia", 0xFF_00FF),
    ("magenta", 0xFF_00FF),
    ("green", 0x00_8000),
    ("lime", 0x00_FF00),
    ("olive", 0x80_8000),
    ("yellow", 0xFF_FF00),
    ("navy", 0x00_0080),
    ("blue", 0x00_00FF),
    ("teal", 0x00_8080),
    ("aqua", 0x00_FFFF),
    ("cyan", 0x00_FFFF),
    ("orange", 0xFF_A500),
    ("pink", 0xFF_C0CB),
    ("brown", 0xA5_2A2A),
    ("gold", 0xFF_D700),
    ("darkblue", 0x00_008B),
    ("darkgreen", 0x00_6400),
    ("darkred", 0x8B_0000),
    ("darkgray", 0xA9_A9A9),
    ("darkgrey", 0xA9_A9A9),
    ("lightgray", 0xD3_D3D3),
    ("lightgrey", 0xD3_D3D3),
    ("lightblue", 0xAD_D8E6),
    ("lightgreen", 0x90_EE90),
    ("lightyellow", 0xFF_FFE0),
    ("indigo", 0x4B_0082),
    ("violet", 0xEE_82EE),
    ("crimson", 0xDC_143C),
    ("tomato", 0xFF_6347),
    ("coral", 0xFF_7F50),
    ("salmon", 0xFA_8072),
    ("khaki", 0xF0_E68C),
    ("beige", 0xF5_F5DC),
    ("ivory", 0xFF_FFF0),
    ("whitesmoke", 0xF5_F5F5),
    ("gainsboro", 0xDC_DCDC),
    ("steelblue", 0x46_82B4),
    ("royalblue", 0x41_69E1),
    ("skyblue", 0x87_CEEB),
    ("turquoise", 0x40_E0D0),
    ("seagreen", 0x2E_8B57),
    ("forestgreen", 0x22_8B22),
    ("chocolate", 0xD2_691E),
    ("sienna", 0xA0_522D),
    ("tan", 0xD2_B48C),
    ("plum", 0xDD_A0DD),
    ("orchid", 0xDA_70D6),
    ("lavender", 0xE6_E6FA),
    ("windowtext", 0x00_0000),
];

/// A CSS colour: `#rgb`, `#rrggbb`, `rgb(r, g, b)` (numbers or percentages),
/// `rgba(…)` (opaque enough) or a keyword. `None` for `transparent`, `inherit`
/// and anything else.
pub fn color(v: &str) -> Option<Color> {
    let v = v.trim().to_ascii_lowercase();
    if let Some(hex) = v.strip_prefix('#') {
        if !hex.is_ascii() {
            return None;
        }
        return match hex.len() {
            3 | 4 => {
                let d = |i: usize| u8::from_str_radix(&hex[i..=i], 16).ok().map(|x| x * 17);
                Some(Color {
                    r: d(0)?,
                    g: d(1)?,
                    b: d(2)?,
                })
            }
            6 | 8 => Color::from_hex(&hex[..6]),
            _ => None,
        };
    }
    if let Some(args) = v
        .strip_prefix("rgba(")
        .or_else(|| v.strip_prefix("rgb("))
        .and_then(|r| r.strip_suffix(')'))
    {
        let parts: Vec<&str> = args
            .split(|c: char| c == ',' || c == '/' || c.is_whitespace())
            .filter(|p| !p.is_empty())
            .collect();
        if parts.len() < 3 {
            return None;
        }
        let ch = |p: &str| -> Option<u8> {
            let x = match p.strip_suffix('%') {
                Some(pc) => pc.parse::<f64>().ok()? * 255.0 / 100.0,
                None => p.parse::<f64>().ok()?,
            };
            x.is_finite().then(|| x.round().clamp(0.0, 255.0) as u8)
        };
        // A fully transparent colour paints nothing.
        if let Some(a) = parts.get(3) {
            if a.trim_end_matches('%').parse::<f64>().ok() == Some(0.0) {
                return None;
            }
        }
        return Some(Color {
            r: ch(parts[0])?,
            g: ch(parts[1])?,
            b: ch(parts[2])?,
        });
    }
    COLORS.iter().find(|(n, _)| *n == v).map(|&(_, rgb)| Color {
        r: (rgb >> 16) as u8,
        g: (rgb >> 8) as u8,
        b: rgb as u8,
    })
}

/// A CSS length in points (CSS Values §5: 1in = 72pt = 96px = 2.54cm; 1pc = 12pt).
/// `em`/`rem`/`%` are relative to `em_pt`; a bare number is pixels (HTML attributes).
pub fn length_pt(v: &str, em_pt: f64) -> Option<f64> {
    let v = v.trim().to_ascii_lowercase();
    let split = v
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '-' | '+')))
        .unwrap_or(v.len());
    let n: f64 = v[..split].parse().ok()?;
    if !n.is_finite() {
        return None;
    }
    let pt = match v[split..].trim() {
        "pt" => n,
        "px" | "" => n * 0.75,
        "in" => n * 72.0,
        "cm" => n * 72.0 / 2.54,
        "mm" => n * 72.0 / 25.4,
        "pc" => n * 12.0,
        "em" | "rem" => n * em_pt,
        "ex" | "ch" => n * em_pt / 2.0,
        "%" => n * em_pt / 100.0,
        _ => return None,
    };
    // Keep the result inside what the document model can hold.
    Some(pt.clamp(-20_000.0, 20_000.0))
}

/// A `font-size` in points: lengths, percentages and the absolute / relative keywords
/// (CSS Fonts §3.5; `medium` = 12pt).
pub fn font_size_pt(v: &str, parent_pt: f64) -> Option<f64> {
    let pt = match v.trim().to_ascii_lowercase().as_str() {
        "xx-small" => 6.75,
        "x-small" => 7.5,
        "small" => 10.0,
        "medium" => 12.0,
        "large" => 13.5,
        "x-large" => 18.0,
        "xx-large" => 24.0,
        "xxx-large" => 36.0,
        "smaller" => parent_pt / 1.2,
        "larger" => parent_pt * 1.2,
        other => length_pt(other, parent_pt)?,
    };
    (pt > 0.0).then(|| pt.clamp(1.0, 1638.0))
}

/// Size in points of the legacy `<font size="1…7">` / `+n` / `-n` attribute
/// (HTML §15.3.1 "Phrasing content", rules for parsing a legacy font size).
pub fn legacy_font_size_pt(v: &str) -> Option<f64> {
    let v = v.trim();
    let n: i32 = v.trim_start_matches('+').parse().ok()?;
    let size = if v.starts_with(['+', '-']) { 3 + n } else { n };
    Some(match size.clamp(1, 7) {
        1 => 7.5,
        2 => 10.0,
        3 => 12.0,
        4 => 13.5,
        5 => 18.0,
        6 => 24.0,
        _ => 36.0,
    })
}

/// First family of a `font-family` list, unquoted; generic families map to a
/// concrete typeface.
pub fn font_family(v: &str) -> Option<String> {
    let first = split_top(v, ',').into_iter().next()?.trim();
    let name = first.trim_matches(|c| c == '"' || c == '\'').trim();
    if name.is_empty() || name.len() > 100 || name.contains(['<', '>', '"']) {
        return None;
    }
    Some(match name.to_ascii_lowercase().as_str() {
        "serif" => "Times New Roman".to_owned(),
        "sans-serif" | "system-ui" => "Arial".to_owned(),
        "monospace" => "Courier New".to_owned(),
        "inherit" | "initial" | "unset" | "cursive" | "fantasy" => return None,
        _ => name.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom;

    #[test]
    fn declarations() {
        let d = parse_decls(
            "Color: RED; font-family: \"A;B\", serif ;margin:0 /* x; y */ ; color: blue !important; bad; :x",
        );
        assert_eq!(d.get("color"), Some("blue"));
        assert_eq!(d.get("font-family"), Some("\"A;B\", serif"));
        assert_eq!(d.get("margin"), Some("0"));
        assert_eq!(d.0.len(), 4);
    }

    #[test]
    fn values() {
        let c = |r, g, b| Some(Color { r, g, b });
        assert_eq!(color("#F80"), c(255, 136, 0));
        assert_eq!(color(" #1a2B3c "), c(0x1a, 0x2b, 0x3c));
        assert_eq!(color("rgb(255, 0, 10)"), c(255, 0, 10));
        assert_eq!(color("rgb(100% 0% 50%)"), c(255, 0, 128));
        assert_eq!(color("rgba(1,2,3,0)"), None);
        assert_eq!(color("Navy"), c(0, 0, 128));
        assert_eq!(color("transparent"), None);
        assert_eq!(color("#ééé"), None);
        assert_eq!(length_pt("12pt", 12.0), Some(12.0));
        assert_eq!(length_pt("16px", 12.0), Some(12.0));
        assert_eq!(length_pt("1in", 12.0), Some(72.0));
        assert_eq!(length_pt("2em", 10.0), Some(20.0));
        assert_eq!(length_pt("150%", 10.0), Some(15.0));
        assert_eq!(length_pt("96", 12.0), Some(72.0));
        assert_eq!(length_pt("auto", 12.0), None);
        assert_eq!(length_pt("1e999pt", 12.0), None);
        assert_eq!(font_size_pt("larger", 10.0), Some(12.0));
        assert_eq!(font_size_pt("0", 10.0), None);
        assert_eq!(legacy_font_size_pt("+2"), Some(18.0));
        assert_eq!(legacy_font_size_pt("7"), Some(36.0));
        assert_eq!(
            font_family("'Times New Roman', serif").as_deref(),
            Some("Times New Roman")
        );
        assert_eq!(font_family("sans-serif").as_deref(), Some("Arial"));
        assert_eq!(font_family("inherit"), None);
    }

    #[test]
    fn style_sheet_cascade() {
        let mut css = StyleSheet::default();
        css.add(
            "<!-- p { color: red; margin: 0 } .note, td.x { color: green } #a { color: blue }
             @media print { p { color: black } } p > b { color: pink } a:hover{color:red}
             p.note.big{font-size:20pt} -->",
        );
        let doc = dom::parse(
            "<p>1</p><p class='note'>2</p><p class='big note' id=a>3</p><p id=a style='color:#000'>4</p><td class=x>5</td>",
        );
        let ps: Vec<&dom::Element> = doc.elements().collect();
        let col = |i: usize| css.computed(ps[i]).get("color").map(str::to_owned);
        assert_eq!(col(0).as_deref(), Some("red"));
        assert_eq!(col(1).as_deref(), Some("green"));
        assert_eq!(col(2).as_deref(), Some("blue"));
        assert_eq!(col(3).as_deref(), Some("#000"));
        assert_eq!(col(4).as_deref(), Some("green"));
        assert_eq!(css.computed(ps[2]).get("font-size"), Some("20pt"));
        assert_eq!(css.computed(ps[0]).get("margin"), Some("0"));
        // Unbalanced input ends quietly.
        css.add("p { color: red");
        css.add("}}}} p {");
    }
}
