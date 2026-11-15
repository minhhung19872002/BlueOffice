//! BlueDoc's ribbon tooltips as a desktop word processor shows them: the command's name
//! (with its shortcut) in semibold, then a short description wrapped under it. The
//! descriptions are Bluestar's own wording (`docTips.*` keys).
use crate::strings::Strings;
use crate::theme::Theme;
use crate::{Painter, TextMeasure, TextStyle};
use bo_render::Rgba;

/// A tooltip ready to draw: title, wrapped description lines, box width (device px).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Tip {
    pub(crate) title: String,
    pub(crate) lines: Vec<String>,
    pub(crate) width: f32,
}

fn style(scale: f32, bold: bool) -> TextStyle {
    TextStyle {
        size: (12.0 * scale).round(),
        bold,
    }
}

/// The description of command `id`, if it has one.
pub(crate) fn description(strings: &Strings, id: &str) -> Option<String> {
    let key = format!("docTips.{}", id.replace('.', "_"));
    let v = strings.get(&key);
    (v != key).then(|| v.to_owned())
}

/// Lay out the tooltip of `title` with `desc` wrapped at the reference's width (about
/// 250 px at 100 %).
pub(crate) fn layout(title: &str, desc: Option<&str>, text: &dyn TextMeasure, scale: f32) -> Tip {
    let title_w = text.measure(title, style(scale, true)).0;
    let max = (250.0 * scale).round();
    let mut lines: Vec<String> = Vec::new();
    let mut widest: f32 = 0.0;
    if let Some(d) = desc {
        let body = style(scale, false);
        let mut line = String::new();
        for word in d.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && text.measure(&candidate, body).0 > max {
                widest = widest.max(text.measure(&line, body).0);
                lines.push(std::mem::replace(&mut line, word.to_owned()));
            } else {
                line = candidate;
            }
        }
        if !line.is_empty() {
            widest = widest.max(text.measure(&line, body).0);
            lines.push(line);
        }
    }
    Tip {
        title: title.to_owned(),
        lines,
        width: title_w.max(widest),
    }
}

/// Draw `tip` under `anchor` (kept inside a window `width` wide): white box, grey frame,
/// soft shadow, title then description lines.
pub(crate) fn paint(
    p: &mut dyn Painter,
    t: &Theme,
    tip: &Tip,
    anchor: [f32; 4],
    width: f32,
    scale: f32,
) {
    let [ax, ay, aw, ah] = anchor;
    let pad = (8.0 * scale).round();
    let line = (12.0 * scale).round() * 1.33;
    let gap = if tip.lines.is_empty() {
        0.0
    } else {
        (6.0 * scale).round()
    };
    let w = (tip.width + 2.0 * pad).ceil();
    let h = (2.0 * (5.0 * scale) + line * (1 + tip.lines.len()) as f32 + gap).round();
    let x = (ax + aw / 2.0 - w / 2.0)
        .clamp(2.0, (width - w - 2.0).max(2.0))
        .round();
    let y = (ay + ah + (6.0 * scale)).round();
    p.fill_rect(x + 1.0, y + 2.0, w, h, Rgba(0, 0, 0, 30));
    p.fill_rect(x, y, w, h, t.border);
    p.fill_rect(x + 1.0, y + 1.0, w - 2.0, h - 2.0, t.surface);
    let top = y + (5.0 * scale).round();
    p.text(&tip.title, x + pad, top, style(scale, true), t.text);
    for (i, l) in tip.lines.iter().enumerate() {
        p.text(
            l,
            x + pad,
            top + line * (i + 1) as f32 + gap,
            style(scale, false),
            t.text,
        );
    }
}
