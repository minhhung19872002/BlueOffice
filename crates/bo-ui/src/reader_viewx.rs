//! BlueReader View tab additions (round 3), the parts that are pure functions: Night Mode
//! colours of a page's display list, the Reflow layout of a page's text, and the ruler ticks.
use bo_pdfread::TextChar;
use bo_render::{Command, PageList, Paint, Rgba};

/// AutoScroll speeds in logical pixels per second; the reference starts at about 2.2 (2.7
/// device pixels a second at 125 %, measured), the arrow keys step through the others.
pub(crate) const AUTO_SPEEDS: [f32; 7] = [1.1, 2.2, 4.4, 8.8, 17.6, 35.2, 70.4];

/// A page's text as plain lines like the reference's Text Viewer: characters grouped by
/// baseline, placed in columns of the page's typical character width (indents and wide gaps
/// become spaces), a blank line where the line spacing opens up.
#[must_use]
pub(crate) fn text_lines(chars: &[TextChar]) -> Vec<String> {
    let items: Vec<&TextChar> = chars
        .iter()
        .filter(|c| !c.text.is_empty() && c.rect[2] > c.rect[0])
        .collect();
    if items.is_empty() {
        return Vec::new();
    }
    let median = |mut v: Vec<f32>| {
        v.sort_by(f32::total_cmp);
        v.get(v.len().saturating_sub(1) / 2)
            .copied()
            .unwrap_or(10.0)
    };
    let size = median(items.iter().map(|c| c.size.max(1.0)).collect()).max(1.0);
    let cw = median(
        items
            .iter()
            .filter(|c| c.text.chars().count() == 1 && !c.text.starts_with(' '))
            .map(|c| c.rect[2] - c.rect[0])
            .collect(),
    )
    .clamp(size * 0.25, size);
    let left = items
        .iter()
        .map(|c| c.rect[0])
        .fold(f32::INFINITY, f32::min);
    // Lines by baseline, top to bottom.
    let mut sorted = items;
    sorted.sort_by(|a, b| a.origin[1].total_cmp(&b.origin[1]));
    let mut lines: Vec<(f32, Vec<&TextChar>)> = Vec::new();
    for c in sorted {
        match lines.last_mut() {
            Some((y, line)) if (c.origin[1] - *y).abs() <= size * 0.4 => line.push(c),
            _ => lines.push((c.origin[1], vec![c])),
        }
    }
    let steps: Vec<f32> = lines.windows(2).map(|w| w[1].0 - w[0].0).collect();
    let pitch = median(steps).max(size);
    let mut out = Vec::new();
    let mut prev_y: Option<f32> = None;
    for (y, mut line) in lines {
        if let Some(py) = prev_y {
            if y - py > pitch * 1.6 {
                out.push(String::new());
            }
        }
        prev_y = Some(y);
        line.sort_by(|a, b| a.rect[0].total_cmp(&b.rect[0]));
        let mut s = String::new();
        let mut len = 0_usize;
        let mut end: Option<f32> = None;
        for c in line {
            let col = ((c.rect[0] - left) / cw).round().max(0.0) as usize;
            let gap = end.map_or(f32::INFINITY, |e| c.rect[0] - e);
            if gap > size * 0.2 && !s.ends_with(' ') && !c.text.starts_with(' ') {
                let pad = col.max(len + usize::from(len > 0)) - len;
                s.extend(std::iter::repeat_n(' ', pad));
                len += pad;
            }
            s.push_str(&c.text);
            len += c.text.chars().count();
            end = Some(c.rect[2]);
        }
        out.push(s.trim_end().to_owned());
    }
    out
}

/// Night Mode paper and text, measured on the reference reader: the page turns (25, 25, 25)
/// and its text (127, 127, 127); other colours stay as they are.
pub(crate) const NIGHT_PAPER: Rgba = Rgba(25, 25, 25, 255);
/// Night Mode text colour.
pub(crate) const NIGHT_TEXT: Rgba = Rgba(127, 127, 127, 255);

/// Night Mode colour of a drawn colour: near-white becomes the paper, near-black the text
/// grey, anything else is kept.
#[must_use]
pub(crate) fn night_color(c: Rgba) -> Rgba {
    let (hi, lo) = (c.0.max(c.1).max(c.2), c.0.min(c.1).min(c.2));
    if lo >= 245 {
        Rgba(NIGHT_PAPER.0, NIGHT_PAPER.1, NIGHT_PAPER.2, c.3)
    } else if hi <= 20 {
        Rgba(NIGHT_TEXT.0, NIGHT_TEXT.1, NIGHT_TEXT.2, c.3)
    } else {
        c
    }
}

fn night_paint(p: &mut Paint) {
    match p {
        Paint::Solid { color } => *color = night_color(*color),
        Paint::Linear { stops, .. } | Paint::Radial { stops, .. } => {
            for s in stops {
                s.color = night_color(s.color);
            }
        }
    }
}

/// The page in Night Mode: a dark sheet, text in grey whatever its colour, white and black
/// graphics turned to the sheet and the text grey; pictures and soft masks are left as
/// they are.
#[must_use]
pub(crate) fn night_list(list: PageList) -> PageList {
    let mut commands = Vec::with_capacity(list.commands.len() + 1);
    commands.push(Command::FillRect {
        x: 0.0,
        y: 0.0,
        w: list.width,
        h: list.height,
        color: NIGHT_PAPER,
    });
    for mut c in list.commands {
        match &mut c {
            Command::Glyphs { color, .. } => {
                *color = Rgba(NIGHT_TEXT.0, NIGHT_TEXT.1, NIGHT_TEXT.2, color.3);
            }
            Command::FillRect { color, .. }
            | Command::Line { color, .. }
            | Command::BlurPath { color, .. } => *color = night_color(*color),
            Command::Path { fill, stroke, .. } => {
                if let Some(f) = fill {
                    night_paint(f);
                }
                if let Some(s) = stroke {
                    s.color = night_color(s.color);
                }
            }
            _ => {}
        }
        commands.push(c);
    }
    PageList {
        width: list.width,
        height: list.height,
        commands,
    }
}

/// Paragraphs of a page's reading text (lines separated by `\n`): lines are joined into a
/// paragraph until an empty line or a line clearly shorter than the page's usual line; a word
/// hyphenated at a line end is joined.
#[must_use]
pub(crate) fn paragraphs(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let mut lens: Vec<usize> = lines
        .iter()
        .map(|l| l.chars().count())
        .filter(|n| *n > 0)
        .collect();
    lens.sort_unstable();
    // The usual full line: the 75th percentile of line lengths.
    let usual = lens.get(lens.len() * 3 / 4).copied().unwrap_or(0);
    let mut out = Vec::new();
    let mut cur = String::new();
    for line in lines {
        if line.is_empty() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }
        if cur.ends_with('-') && cur.chars().rev().nth(1).is_some_and(char::is_alphabetic) {
            cur.pop();
        } else if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(line);
        let n = line.chars().count();
        if (n as f32) < usual as f32 * 0.7 && !line.ends_with('-') {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// Lines of `paragraphs` wrapped to `width` pixels (`measure` gives a text's width), each
/// paragraph followed by a blank line; words wider than a line are broken by characters.
#[must_use]
pub(crate) fn wrap(
    paragraphs: &[String],
    width: f32,
    measure: &dyn Fn(&str) -> f32,
) -> Vec<String> {
    let mut out = Vec::new();
    for p in paragraphs {
        let mut line = String::new();
        for word in p.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if measure(&candidate) <= width || line.is_empty() && measure(word) <= width {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                out.push(std::mem::take(&mut line));
            }
            // A word longer than the line: break it by characters.
            for ch in word.chars() {
                line.push(ch);
                if measure(&line) > width && line.chars().count() > 1 {
                    let last = line.pop().unwrap_or(' ');
                    out.push(std::mem::take(&mut line));
                    line.push(last);
                }
            }
        }
        if !line.is_empty() {
            out.push(line);
        }
        out.push(String::new());
    }
    out
}

/// Ruler ticks from `start` to `end` pixels for a page origin at `origin`, `px_per_unit`
/// pixels per unit (inch / cm): (position, level) with level 0 a labelled whole unit, 1 a
/// half and 2 a smaller step.
#[must_use]
pub(crate) fn ruler_ticks(
    origin: f32,
    px_per_unit: f32,
    start: f32,
    end: f32,
) -> Vec<(f32, u8, i32)> {
    if px_per_unit <= 0.0 {
        return Vec::new();
    }
    // Subdivide so ticks stay at least 5 px apart.
    let sub = if px_per_unit / 8.0 >= 5.0 {
        8
    } else if px_per_unit / 4.0 >= 5.0 {
        4
    } else if px_per_unit / 2.0 >= 5.0 {
        2
    } else {
        1
    };
    let step = px_per_unit / sub as f32;
    let first = ((start - origin) / step).floor() as i32;
    let last = ((end - origin) / step).ceil() as i32;
    (first..=last)
        .map(|k| {
            let x = origin + k as f32 * step;
            let level = if k.rem_euclid(sub) == 0 {
                0
            } else if sub >= 2 && k.rem_euclid(sub) == sub / 2 {
                1
            } else {
                2
            };
            (x, level, k.div_euclid(sub))
        })
        .filter(|(x, _, _)| *x >= start && *x <= end)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn night_mode_dark_paper_grey_text() {
        assert_eq!(night_color(Rgba(255, 255, 255, 9)), Rgba(25, 25, 25, 9));
        assert_eq!(night_color(Rgba(0, 0, 0, 255)), NIGHT_TEXT);
        assert_eq!(night_color(Rgba(255, 127, 0, 255)), Rgba(255, 127, 0, 255));
        let list = PageList {
            width: 10.0,
            height: 20.0,
            commands: vec![Command::Glyphs {
                face: 0,
                size: 10.0,
                x: 0.0,
                y: 0.0,
                color: Rgba(200, 0, 0, 255),
                text: "a".into(),
                glyphs: Vec::new(),
            }],
        };
        let n = night_list(list);
        assert!(matches!(
            n.commands[0],
            Command::FillRect { w, color: NIGHT_PAPER, .. } if w == 10.0
        ));
        assert!(matches!(
            n.commands[1],
            Command::Glyphs {
                color: NIGHT_TEXT,
                ..
            }
        ));
    }

    #[test]
    fn reflow_joins_lines_and_wraps() {
        let text = "The quick brown fox jumps over\nthe lazy dog and runs away from\nthe hun-\nters.\n\nNext para\ngraph here.";
        let p = paragraphs(text);
        assert_eq!(p.len(), 3);
        assert_eq!(
            p[0],
            "The quick brown fox jumps over the lazy dog and runs away from the hunters."
        );
        let measure = |s: &str| s.chars().count() as f32 * 10.0;
        let lines = wrap(&p[..1], 200.0, &measure);
        assert!(lines.iter().all(|l| l.chars().count() <= 20));
        assert_eq!(lines.last().map(String::as_str), Some(""));
        assert_eq!(lines[0], "The quick brown fox");
        // A long word breaks.
        let lines = wrap(&["abcdefghijklmnopqrstuvwxyz".to_owned()], 100.0, &measure);
        assert_eq!(lines[0], "abcdefghij");
    }

    #[test]
    fn text_viewer_lines_keep_columns_and_paragraph_gaps() {
        let ch = |t: &str, x: f32, y: f32| bo_pdfread::TextChar {
            text: t.to_owned(),
            rect: [x, y - 8.0, x + 5.0, y + 2.0],
            origin: [x, y],
            dir: [1.0, 0.0],
            size: 10.0,
        };
        let mut chars = Vec::new();
        // "ab" at the margin, "cd" indented by four columns on the next line, a paragraph gap,
        // then "e f" with a wide gap.
        chars.extend([ch("a", 100.0, 100.0), ch("b", 105.0, 100.0)]);
        chars.extend([ch("c", 120.0, 112.0), ch("d", 125.0, 112.0)]);
        chars.extend([ch("e", 100.0, 150.0), ch("f", 130.0, 150.0)]);
        let lines = text_lines(&chars);
        assert_eq!(lines, vec!["ab", "    cd", "", "e     f"]);
    }

    #[test]
    fn ruler_ticks_by_unit() {
        // 96 px per inch: eighths are 12 px apart.
        let t = ruler_ticks(100.0, 96.0, 100.0, 292.0);
        assert_eq!(t.first(), Some(&(100.0, 0, 0)));
        assert_eq!(t.len(), 17);
        assert_eq!(t[4], (148.0, 1, 0));
        assert_eq!(t[8], (196.0, 0, 1));
        // Left of the page: negative units.
        let t = ruler_ticks(100.0, 96.0, 0.0, 10.0);
        assert!(t.iter().all(|(_, _, u)| *u == -2 || *u == -1));
    }
}
