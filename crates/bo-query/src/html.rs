//! Web tables: the `<table>` elements of an HTML page (WHATWG HTML, §4.9 tabular data).
//!
//! A tolerant tokenizer, enough for tables: tags and their attributes, text with character
//! references, `<script>`/`<style>`/comments skipped. Each `<tr>` of a table (not of a table
//! nested in it) is a row; `<td>`/`<th>` are cells (`colspan` repeats the value, `rowspan`
//! carries it to the next rows); `<br>` is a line break. Nested tables are tables of their
//! own, listed after their parent.

use crate::value::{QTable, Val};

/// A table found on a page.
#[derive(Debug, Clone, PartialEq)]
pub struct WebTable {
    /// `<caption>` text, else the `id` or `summary`/`aria-label` attribute.
    pub caption: String,
    /// The first row is made of `<th>` cells.
    pub header: bool,
    /// Cell texts.
    pub rows: Vec<Vec<String>>,
}

impl WebTable {
    /// The table as query rows (`Column1…`).
    pub fn to_table(&self) -> QTable {
        QTable::from_rows(
            self.rows
                .iter()
                .map(|r| {
                    r.iter()
                        .map(|c| {
                            if c.is_empty() {
                                Val::Null
                            } else {
                                Val::Text(c.clone())
                            }
                        })
                        .collect()
                })
                .collect(),
        )
    }
}

#[derive(Default)]
struct Building {
    caption: String,
    label: String,
    rows: Vec<Vec<String>>,
    header_flags: Vec<bool>,
    row: Option<(Vec<String>, bool)>,
    cell: Option<(String, usize, usize, bool)>,
    in_caption: bool,
    /// (column, remaining rows, text) carried down by rowspan.
    spans: Vec<(usize, usize, String)>,
}

impl Building {
    fn end_cell(&mut self) {
        if let Some((text, colspan, rowspan, th)) = self.cell.take() {
            let text = collapse(&text);
            let row = self.row.get_or_insert_with(|| (Vec::new(), true));
            row.1 &= th;
            for _ in 0..colspan.clamp(1, 1000) {
                // Leave room for cells carried down by rowspan.
                while let Some(pos) = self
                    .spans
                    .iter()
                    .position(|s| s.0 == row.0.len() && s.1 > 0)
                {
                    let s = &mut self.spans[pos];
                    row.0.push(s.2.clone());
                    s.1 -= 1;
                }
                if rowspan > 1 {
                    self.spans.push((row.0.len(), rowspan - 1, text.clone()));
                }
                row.0.push(text.clone());
            }
        }
    }

    fn end_row(&mut self) {
        self.end_cell();
        if let Some((mut cells, th)) = self.row.take() {
            while let Some(pos) = self
                .spans
                .iter()
                .position(|s| s.0 == cells.len() && s.1 > 0)
            {
                let s = &mut self.spans[pos];
                cells.push(s.2.clone());
                s.1 -= 1;
            }
            self.spans.retain(|s| s.1 > 0);
            if !cells.is_empty() {
                self.header_flags.push(th);
                self.rows.push(cells);
            }
        }
    }

    fn finish(mut self) -> WebTable {
        self.end_row();
        let caption = if self.caption.trim().is_empty() {
            self.label
        } else {
            collapse(&self.caption)
        };
        WebTable {
            caption,
            header: self.header_flags.first().copied().unwrap_or(false),
            rows: self.rows,
        }
    }
}

fn collapse(s: &str) -> String {
    let lines: Vec<String> = s
        .split('\n')
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    lines
        .into_iter()
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Character references (`&amp;`, `&#233;`, `&#xE9;`, the common named ones).
pub fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 1..];
        let end = tail.find(';').filter(|&e| e <= 10);
        let rep = end.and_then(|e| {
            let ent = &tail[..e];
            let c = match ent {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                "ndash" => Some('–'),
                "mdash" => Some('—'),
                "hellip" => Some('…'),
                "copy" => Some('©'),
                "reg" => Some('®'),
                "euro" => Some('€'),
                _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                    u32::from_str_radix(&ent[2..], 16)
                        .ok()
                        .and_then(char::from_u32)
                }
                _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
                _ => None,
            };
            c.map(|c| (c, e))
        });
        if let Some((c, e)) = rep {
            out.push(c);
            rest = &tail[e + 1..];
        } else {
            out.push('&');
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

struct Tag {
    name: String,
    end: bool,
    attrs: Vec<(String, String)>,
}

fn parse_tag(s: &str) -> Option<Tag> {
    let (end, body) = match s.strip_prefix('/') {
        Some(b) => (true, b),
        None => (false, s),
    };
    let body = body.trim_end_matches('/');
    let name_end = body
        .find(|c: char| c.is_whitespace() || c == '/')
        .unwrap_or(body.len());
    let name = body[..name_end].to_ascii_lowercase();
    if name.is_empty() || !name.chars().next().is_some_and(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let mut attrs = Vec::new();
    let mut rest = body[name_end..].trim_start();
    while !rest.is_empty() {
        let k_end = rest
            .find(|c: char| c.is_whitespace() || c == '=' || c == '/')
            .unwrap_or(rest.len());
        let key = rest[..k_end].to_ascii_lowercase();
        rest = rest[k_end..].trim_start();
        let mut value = String::new();
        if let Some(r) = rest.strip_prefix('=') {
            let r = r.trim_start();
            if let Some(q) = r.chars().next().filter(|c| *c == '"' || *c == '\'') {
                let inner = &r[1..];
                let close = inner.find(q).unwrap_or(inner.len());
                value = decode_entities(&inner[..close]);
                rest = inner.get(close + 1..).unwrap_or("");
            } else {
                let v_end = r.find(char::is_whitespace).unwrap_or(r.len());
                value = decode_entities(&r[..v_end]);
                rest = &r[v_end..];
            }
        } else if k_end == 0 {
            rest = rest.get(1..).unwrap_or("");
        }
        if !key.is_empty() {
            attrs.push((key, value));
        }
        rest = rest.trim_start();
    }
    Some(Tag { name, end, attrs })
}

fn attr<'a>(t: &'a Tag, k: &str) -> Option<&'a str> {
    t.attrs
        .iter()
        .find(|(n, _)| n == k)
        .map(|(_, v)| v.as_str())
}

/// The tables of an HTML page, in document order of their start tags.
pub fn tables(html: &str) -> Vec<WebTable> {
    let mut done: Vec<(usize, WebTable)> = Vec::new();
    let mut open: Vec<(usize, Building)> = Vec::new();
    let mut seq = 0usize;
    let mut rest = html;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            text(&mut open, rest);
            break;
        };
        text(&mut open, &rest[..lt]);
        let tail = &rest[lt + 1..];
        if let Some(c) = tail.strip_prefix("!--") {
            rest = c.find("-->").map_or("", |e| &c[e + 3..]);
            continue;
        }
        if tail.starts_with('!') || tail.starts_with('?') {
            rest = tail.find('>').map_or("", |e| &tail[e + 1..]);
            continue;
        }
        let Some(gt) = find_tag_end(tail) else {
            text(&mut open, &rest[lt..]);
            break;
        };
        let raw = &tail[..gt];
        rest = &tail[gt + 1..];
        let Some(tag) = parse_tag(raw) else {
            text(&mut open, &format!("<{raw}>"));
            continue;
        };
        if !tag.end && (tag.name == "script" || tag.name == "style" || tag.name == "template") {
            let close = format!("</{}", tag.name);
            let lower = rest.to_ascii_lowercase();
            rest = lower.find(&close).map_or("", |i| {
                let after = &rest[i..];
                after.find('>').map_or("", |e| &after[e + 1..])
            });
            continue;
        }
        match (tag.name.as_str(), tag.end) {
            ("table", false) => {
                let label = attr(&tag, "id")
                    .or_else(|| attr(&tag, "aria-label"))
                    .or_else(|| attr(&tag, "summary"))
                    .unwrap_or_default()
                    .to_owned();
                open.push((
                    seq,
                    Building {
                        label,
                        ..Building::default()
                    },
                ));
                seq += 1;
            }
            ("table", true) => {
                if let Some((n, b)) = open.pop() {
                    done.push((n, b.finish()));
                }
            }
            ("caption", false) => {
                if let Some((_, b)) = open.last_mut() {
                    b.in_caption = true;
                }
            }
            ("caption", true) => {
                if let Some((_, b)) = open.last_mut() {
                    b.in_caption = false;
                }
            }
            ("tr", false) => {
                if let Some((_, b)) = open.last_mut() {
                    b.end_row();
                    b.row = Some((Vec::new(), true));
                }
            }
            ("tr", true) | ("thead" | "tbody" | "tfoot", _) => {
                if let Some((_, b)) = open.last_mut() {
                    b.end_row();
                }
            }
            ("td" | "th", false) => {
                if let Some((_, b)) = open.last_mut() {
                    b.end_cell();
                    let span = |k: &str| {
                        attr(&tag, k)
                            .and_then(|v| v.trim().parse::<usize>().ok())
                            .unwrap_or(1)
                    };
                    b.cell = Some((
                        String::new(),
                        span("colspan"),
                        span("rowspan").min(1000),
                        tag.name == "th",
                    ));
                }
            }
            ("td" | "th", true) => {
                if let Some((_, b)) = open.last_mut() {
                    b.end_cell();
                }
            }
            ("br" | "p" | "div" | "li", _) => text(&mut open, "\n"),
            _ => {}
        }
    }
    while let Some((n, b)) = open.pop() {
        done.push((n, b.finish()));
    }
    done.sort_by_key(|(n, _)| *n);
    done.into_iter().map(|(_, t)| t).collect()
}

fn find_tag_end(s: &str) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (i, c) in s.char_indices() {
        match quote {
            Some(q) if c == q => quote = None,
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '>' => return Some(i),
            None if c == '<' && i > 0 => return None,
            _ => {}
        }
    }
    None
}

fn text(open: &mut [(usize, Building)], s: &str) {
    let Some((_, b)) = open.last_mut() else {
        return;
    };
    let t = decode_entities(s);
    if b.in_caption {
        b.caption.push_str(&t);
    } else if let Some(cell) = b.cell.as_mut() {
        cell.0.push_str(&t);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_cells_and_spans() {
        let html = r#"<html><head><style>td{}</style><script>var t="<table>";</script></head><body>
<table id="prices"><caption>Giá &amp; số lượng</caption>
<thead><tr><th>Mặt hàng</th><th colspan=2>Giá</th></tr></thead>
<tbody><tr><td rowspan="2">Táo</td><td>1</td><td>2</td></tr>
<tr><td>3<br>x</td><td>4</td>
<tr><td>Lê<td>&#55;<td>8
</table>
<!-- <table><tr><td>no</td></tr></table> -->
<table><tr><td>a</td><td><table><tr><td>inner</td></tr></table></td></tr></table>
</body></html>"#;
        let ts = tables(html);
        assert_eq!(ts.len(), 3);
        assert_eq!(ts[0].caption, "Giá & số lượng");
        assert!(ts[0].header);
        assert_eq!(ts[0].rows[0], vec!["Mặt hàng", "Giá", "Giá"]);
        assert_eq!(ts[0].rows[2], vec!["Táo", "3\nx", "4"]);
        assert_eq!(ts[0].rows[3], vec!["Lê", "7", "8"]);
        assert_eq!(ts[1].rows[0], vec!["a", ""]);
        assert_eq!(ts[2].rows[0], vec!["inner"]);
        assert_eq!(ts[0].to_table().columns.len(), 3);
    }
}
