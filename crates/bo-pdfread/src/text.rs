//! Text layer of a page: shown characters in content order with the spaces and line breaks
//! their positions imply, for search, selection and copy.
use crate::TextChar;

/// Characters of a page with the text they read as.
#[derive(Debug, Clone, Default)]
pub struct TextLayer {
    chars: Vec<TextChar>,
    /// Reading text: (character, index of its `TextChar`, or `usize::MAX` for a separator).
    seq: Vec<(char, usize)>,
}

fn baseline_frame(c: &TextChar) -> ([f32; 2], f32) {
    let len = (c.dir[0] * c.dir[0] + c.dir[1] * c.dir[1]).sqrt();
    if len < 1e-6 {
        ([1.0, 0.0], 1.0)
    } else {
        ([c.dir[0] / len, c.dir[1] / len], len)
    }
}

impl TextLayer {
    /// Build the layer from a page's characters.
    pub fn new(chars: Vec<TextChar>) -> Self {
        let mut seq = Vec::with_capacity(chars.len() * 2);
        for (i, c) in chars.iter().enumerate() {
            if i > 0 {
                let p = &chars[i - 1];
                let (d, _) = baseline_frame(p);
                // Across the previous character's baseline; gap along it.
                let across = |q: &[f32; 2]| -q[0] * d[1] + q[1] * d[0];
                let size = p.size.max(c.size).max(1.0);
                let dy = (across(&c.origin) - across(&p.origin)).abs();
                let gap = if d[0].abs() >= d[1].abs() {
                    if d[0] > 0.0 {
                        c.rect[0] - p.rect[2]
                    } else {
                        p.rect[0] - c.rect[2]
                    }
                } else if d[1] > 0.0 {
                    c.rect[1] - p.rect[3]
                } else {
                    p.rect[1] - c.rect[3]
                };
                let last = seq.last().map(|s: &(char, usize)| s.0);
                let ends_white = last.is_some_and(char::is_whitespace);
                let starts_white = c.text.starts_with(char::is_whitespace);
                if dy > size * 0.5 || gap < -size * 1.5 {
                    if last != Some('\n') {
                        // A hyphen at the end of a line joins the word.
                        seq.push(('\n', usize::MAX));
                    }
                } else if gap > size * 0.15 && !ends_white && !starts_white {
                    seq.push((' ', usize::MAX));
                }
            }
            for ch in c.text.chars() {
                // Drop control characters and the soft hyphen marks of some producers.
                if ch.is_control() && ch != '\n' {
                    continue;
                }
                seq.push((ch, i));
            }
        }
        Self { chars, seq }
    }

    /// Number of characters.
    pub fn len(&self) -> usize {
        self.chars.len()
    }

    /// No text on the page.
    pub fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    /// The characters.
    pub fn chars(&self) -> &[TextChar] {
        &self.chars
    }

    /// All text of the page.
    pub fn text(&self) -> String {
        self.seq.iter().map(|s| s.0).collect()
    }

    /// Text of characters `a..=b` (in content order).
    pub fn slice(&self, a: usize, b: usize) -> String {
        let (a, b) = (a.min(b), a.max(b));
        let mut out = String::new();
        let mut inside = false;
        for &(ch, i) in &self.seq {
            if i != usize::MAX {
                if i > b {
                    break;
                }
                inside = i >= a;
            }
            if inside {
                out.push(ch);
            }
        }
        out.trim_end().to_owned()
    }

    /// Ranges of characters (first, last) matching `needle`; white space matches any run of
    /// spaces or line breaks.
    pub fn find(&self, needle: &str, match_case: bool, whole_word: bool) -> Vec<(usize, usize)> {
        let norm = |c: char| -> char {
            if c.is_whitespace() {
                ' '
            } else if match_case {
                c
            } else {
                c.to_lowercase().next().unwrap_or(c)
            }
        };
        let mut pat: Vec<char> = Vec::new();
        for c in needle.chars().map(norm) {
            if c == ' ' && pat.last() == Some(&' ') {
                continue;
            }
            pat.push(c);
        }
        while pat.last() == Some(&' ') {
            pat.pop();
        }
        if pat.is_empty() {
            return Vec::new();
        }
        let hay: Vec<(char, usize)> = self.seq.iter().map(|&(c, i)| (norm(c), i)).collect();
        let mut out = Vec::new();
        let mut s = 0;
        while s < hay.len() {
            let mut k = 0;
            let mut j = s;
            while k < pat.len() && j < hay.len() {
                if pat[k] == ' ' {
                    if hay[j].0 != ' ' {
                        break;
                    }
                    while j < hay.len() && hay[j].0 == ' ' {
                        j += 1;
                    }
                    k += 1;
                    continue;
                }
                if hay[j].0 != pat[k] {
                    break;
                }
                j += 1;
                k += 1;
            }
            if k == pat.len() {
                let word = |c: char| c.is_alphanumeric();
                let ok = !whole_word
                    || ((s == 0 || !word(hay[s - 1].0)) && (j >= hay.len() || !word(hay[j].0)));
                let first = hay[s..j].iter().find(|h| h.1 != usize::MAX).map(|h| h.1);
                let last = hay[s..j]
                    .iter()
                    .rev()
                    .find(|h| h.1 != usize::MAX)
                    .map(|h| h.1);
                if let (true, Some(a), Some(b)) = (ok, first, last) {
                    out.push((a, b));
                    s = j;
                    continue;
                }
            }
            s += 1;
        }
        out
    }

    /// Highlight rectangles of characters `a..=b`, one per line piece.
    pub fn rects(&self, a: usize, b: usize) -> Vec<[f32; 4]> {
        let (a, b) = (a.min(b), a.max(b));
        let mut out: Vec<[f32; 4]> = Vec::new();
        let mut prev: Option<&TextChar> = None;
        for c in self.chars.iter().take(b + 1).skip(a) {
            let r = c.rect;
            let same_line = prev.is_some_and(|p| {
                let h = (p.rect[3] - p.rect[1]).max(r[3] - r[1]);
                (p.origin[1] - c.origin[1]).abs() < h * 0.5 && r[0] >= p.rect[0] - h
            });
            match (same_line, out.last_mut()) {
                (true, Some(last)) => {
                    last[0] = last[0].min(r[0]);
                    last[1] = last[1].min(r[1]);
                    last[2] = last[2].max(r[2]);
                    last[3] = last[3].max(r[3]);
                }
                _ => out.push(r),
            }
            prev = Some(c);
        }
        out
    }

    /// The character at or nearest to a point (page points) on the same line, or the nearest
    /// character overall within `reach` points.
    pub fn hit(&self, x: f32, y: f32, reach: f32) -> Option<usize> {
        let mut best: Option<(f32, usize)> = None;
        for (i, c) in self.chars.iter().enumerate() {
            let r = c.rect;
            let dx = if x < r[0] {
                r[0] - x
            } else if x > r[2] {
                x - r[2]
            } else {
                0.0
            };
            let dy = if y < r[1] {
                r[1] - y
            } else if y > r[3] {
                y - r[3]
            } else {
                0.0
            };
            // Prefer the same line: vertical distance weighs more.
            let d = dx + dy * 4.0;
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, i));
            }
        }
        best.filter(|b| b.0 <= reach).map(|b| b.1)
    }

    /// Whether a point is over text.
    pub fn over_text(&self, x: f32, y: f32) -> bool {
        self.chars
            .iter()
            .any(|c| x >= c.rect[0] && x <= c.rect[2] && y >= c.rect[1] && y <= c.rect[3])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(t: &str, x: f32, y: f32) -> TextChar {
        TextChar {
            text: t.into(),
            rect: [x, y - 8.0, x + 5.0, y + 2.0],
            origin: [x, y],
            dir: [10.0, 0.0],
            size: 10.0,
        }
    }

    #[test]
    fn spaces_lines_search_and_rects() {
        let l = TextLayer::new(vec![
            ch("H", 0.0, 10.0),
            ch("i", 5.0, 10.0),
            ch("t", 15.0, 10.0),
            ch("o", 20.0, 10.0),
            ch("y", 0.0, 30.0),
            ch("o", 5.0, 30.0),
        ]);
        assert_eq!(l.text(), "Hi to\nyo");
        assert_eq!(l.find("TO YO", false, false), vec![(2, 5)]);
        assert_eq!(l.find("hi", true, false), Vec::<(usize, usize)>::new());
        assert_eq!(l.find("o", false, true), Vec::<(usize, usize)>::new());
        assert_eq!(l.slice(1, 4), "i to\ny");
        assert_eq!(l.rects(0, 5).len(), 2);
        assert_eq!(l.hit(16.0, 9.0, 5.0), Some(2));
    }
}
