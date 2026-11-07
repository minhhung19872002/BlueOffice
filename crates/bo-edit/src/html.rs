//! HTML clipboard format: export of copied paragraphs and a tolerant importer
//! for HTML pasted from browsers and other applications (inline formatting,
//! paragraphs, line breaks, simple lists and tables as tab-separated lines).

use crate::effective_run;
use crate::para::{self, Atom};
use bo_model::{
    Alignment, Block, Color, Document, HalfPoints, ParaProps, Paragraph, RunContent, RunProps,
    Underline, VertAlign,
};

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            _ => o.push(c),
        }
    }
    o
}

fn css_of(r: &RunProps) -> String {
    let mut css = Vec::new();
    if let Some(f) = &r.font {
        css.push(format!("font-family:'{}'", f.replace('\'', "")));
    }
    if let Some(s) = r.size {
        css.push(format!("font-size:{}pt", s.to_pt()));
    }
    if r.bold == Some(true) {
        css.push("font-weight:bold".into());
    }
    if r.italic == Some(true) {
        css.push("font-style:italic".into());
    }
    let mut deco = Vec::new();
    if r.underline.is_some_and(|u| u != Underline::None) {
        deco.push("underline");
    }
    if r.strike == Some(true) || r.double_strike == Some(true) {
        deco.push("line-through");
    }
    if !deco.is_empty() {
        css.push(format!("text-decoration:{}", deco.join(" ")));
    }
    if let Some(c) = r.color {
        css.push(format!("color:#{}", c.to_hex()));
    }
    if let Some(c) = r.highlight.or(r.shading) {
        css.push(format!("background-color:#{}", c.to_hex()));
    }
    css.join(";")
}

/// Serialize paragraphs as an HTML fragment (effective formatting inlined as CSS).
pub fn to_html(doc: &Document, blocks: &[Block]) -> String {
    let mut out = String::from("<meta charset=\"utf-8\"><div>");
    for b in blocks {
        let Block::Paragraph(p) = b else { continue };
        let align = match p.props.align {
            Some(Alignment::Center) => "center",
            Some(Alignment::Right) => "right",
            Some(Alignment::Justify) => "justify",
            _ => "left",
        };
        out.push_str(&format!("<p style=\"margin:0;text-align:{align}\">"));
        for r in &p.runs {
            let eff = effective_run(doc, p, &r.props);
            let (open, close) = match eff.vert_align {
                Some(VertAlign::Superscript) => ("<sup>", "</sup>"),
                Some(VertAlign::Subscript) => ("<sub>", "</sub>"),
                _ => ("", ""),
            };
            let mut inner = String::new();
            for c in &r.content {
                match c {
                    RunContent::Text(t) => inner.push_str(&esc(t)),
                    RunContent::Tab => inner.push_str("<span style=\"white-space:pre\">\t</span>"),
                    RunContent::LineBreak => inner.push_str("<br>"),
                    RunContent::NoBreakHyphen => inner.push('\u{2011}'),
                    RunContent::Field { cached, .. } => inner.push_str(&esc(cached)),
                    _ => {}
                }
            }
            if inner.is_empty() {
                continue;
            }
            let span = format!(
                "{open}<span style=\"{}\">{inner}</span>{close}",
                css_of(&eff)
            );
            match &eff.link {
                Some(l) if !l.starts_with('#') => {
                    out.push_str(&format!("<a href=\"{}\">{span}</a>", esc(l)));
                }
                _ => out.push_str(&span),
            }
        }
        out.push_str("</p>");
    }
    out.push_str("</div>");
    out
}

// ------------------------------------------------------------------ import

fn entity(name: &str) -> Option<char> {
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{00A0}',
        "ndash" => '–',
        "mdash" => '—',
        "hellip" => '…',
        "laquo" => '«',
        "raquo" => '»',
        "ldquo" => '“',
        "rdquo" => '”',
        "lsquo" => '‘',
        "rsquo" => '’',
        "copy" => '©',
        "reg" => '®',
        "trade" => '™',
        "euro" => '€',
        n if n.starts_with("#x") || n.starts_with("#X") => {
            char::from_u32(u32::from_str_radix(&n[2..], 16).ok()?)?
        }
        n if n.starts_with('#') => char::from_u32(n[1..].parse().ok()?)?,
        _ => return None,
    })
}

fn decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if let Some((j, c)) = tail
            .find(';')
            .filter(|&j| j <= 10)
            .and_then(|j| Some((j, entity(&tail[1..j])?)))
        {
            out.push(c);
            rest = &tail[j + 1..];
        } else {
            out.push('&');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

fn parse_color(v: &str) -> Option<Color> {
    let v = v.trim();
    if let Some(h) = v.strip_prefix('#') {
        return match h.len() {
            6 => Color::from_hex(h),
            3 => {
                let x: String = h.chars().flat_map(|c| [c, c]).collect();
                Color::from_hex(&x)
            }
            _ => None,
        };
    }
    if let Some(args) = v.strip_prefix("rgb(").or_else(|| v.strip_prefix("rgba(")) {
        let n: Vec<u8> = args
            .trim_end_matches(')')
            .split(',')
            .take(3)
            .filter_map(|x| {
                x.trim()
                    .parse::<f32>()
                    .ok()
                    .map(|f| f.clamp(0.0, 255.0) as u8)
            })
            .collect();
        if let [r, g, b] = n[..] {
            return Some(Color { r, g, b });
        }
    }
    match v.to_ascii_lowercase().as_str() {
        "black" => Some(Color::BLACK),
        "white" => Some(Color::WHITE),
        "red" => Some(Color { r: 255, g: 0, b: 0 }),
        "blue" => Some(Color { r: 0, g: 0, b: 255 }),
        "green" => Some(Color { r: 0, g: 128, b: 0 }),
        "yellow" => Some(Color {
            r: 255,
            g: 255,
            b: 0,
        }),
        _ => None,
    }
}

fn apply_css(style: &str, r: &mut RunProps) {
    for decl in style.split(';') {
        let Some((k, v)) = decl.split_once(':') else {
            continue;
        };
        let k = k.trim().to_ascii_lowercase();
        let v = v.trim();
        let lv = v.to_ascii_lowercase();
        match k.as_str() {
            "font-weight" => {
                r.bold = Some(lv.contains("bold") || lv.parse::<u32>().is_ok_and(|w| w >= 600));
            }
            "font-style" => r.italic = Some(lv.contains("italic") || lv.contains("oblique")),
            "text-decoration" | "text-decoration-line" => {
                if lv.contains("underline") {
                    r.underline = Some(Underline::Single);
                }
                if lv.contains("line-through") {
                    r.strike = Some(true);
                }
                if lv == "none" {
                    r.underline = None;
                    r.strike = None;
                }
            }
            "color" => r.color = parse_color(v),
            "background-color" | "background" => {
                r.highlight = parse_color(v).filter(|c| *c != Color::WHITE);
            }
            "font-size" => {
                let pt = if let Some(n) = lv.strip_suffix("pt") {
                    n.trim().parse::<f32>().ok()
                } else if let Some(n) = lv.strip_suffix("px") {
                    n.trim().parse::<f32>().ok().map(|px| px * 0.75)
                } else {
                    None
                };
                if let Some(pt) = pt.filter(|p| *p > 1.0 && *p < 1000.0) {
                    r.size = Some(HalfPoints((pt * 2.0).round() as u32));
                }
            }
            "font-family" => {
                let f = v
                    .split(',')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_matches(['\'', '"']);
                if !f.is_empty() {
                    r.font = Some(f.to_owned());
                }
            }
            "vertical-align" => match lv.as_str() {
                "super" => r.vert_align = Some(VertAlign::Superscript),
                "sub" => r.vert_align = Some(VertAlign::Subscript),
                _ => {}
            },
            _ => {}
        }
    }
}

fn attr_of(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let mut search = 0;
    while let Some(i) = lower[search..].find(name) {
        let at = search + i;
        let before_ok = at == 0 || lower.as_bytes()[at - 1].is_ascii_whitespace();
        let rest = tag[at + name.len()..].trim_start();
        if before_ok {
            if let Some(rest) = rest.strip_prefix('=') {
                let rest = rest.trim_start();
                let (q, body) = match rest.chars().next() {
                    Some(q @ ('"' | '\'')) => (Some(q), &rest[1..]),
                    _ => (None, rest),
                };
                let end = match q {
                    Some(q) => body.find(q).unwrap_or(body.len()),
                    None => body
                        .find(|c: char| c.is_whitespace() || c == '>')
                        .unwrap_or(body.len()),
                };
                return Some(decode(&body[..end]));
            }
        }
        search = at + name.len();
    }
    None
}

struct Importer {
    blocks: Vec<Block>,
    atoms: Vec<Atom>,
    para: ParaProps,
    stack: Vec<(String, RunProps)>,
    lists: Vec<(bool, u32)>,
    skip: usize,
    pending_space: bool,
}

impl Importer {
    fn props(&self) -> RunProps {
        self.stack.last().map(|s| s.1.clone()).unwrap_or_default()
    }

    fn flush(&mut self) {
        if self.atoms.is_empty() {
            return;
        }
        let atoms = std::mem::take(&mut self.atoms);
        self.blocks.push(Block::Paragraph(Paragraph::new(
            std::mem::take(&mut self.para),
            para::rebuild(atoms),
        )));
        self.pending_space = false;
    }

    fn text(&mut self, raw: &str) {
        let props = self.props();
        let mut s = String::new();
        for c in decode(raw).chars() {
            if c.is_whitespace() && c != '\u{00A0}' {
                self.pending_space = true;
                continue;
            }
            if self.pending_space && (!self.atoms.is_empty() || !s.is_empty()) {
                s.push(' ');
            }
            self.pending_space = false;
            s.push(c);
        }
        if !s.is_empty() {
            self.atoms.push((props, RunContent::Text(s)));
        }
    }

    fn open(&mut self, name: &str, tag: &str) {
        let mut r = self.props();
        match name {
            "b" | "strong" => r.bold = Some(true),
            "i" | "em" | "cite" => r.italic = Some(true),
            "u" | "ins" => r.underline = Some(Underline::Single),
            "s" | "strike" | "del" => r.strike = Some(true),
            "sup" => r.vert_align = Some(VertAlign::Superscript),
            "sub" => r.vert_align = Some(VertAlign::Subscript),
            "a" => r.link = attr_of(tag, "href"),
            "font" => {
                if let Some(c) = attr_of(tag, "color").as_deref().and_then(parse_color) {
                    r.color = Some(c);
                }
                if let Some(f) = attr_of(tag, "face") {
                    r.font = Some(f);
                }
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                r.bold = Some(true);
                let size = [48, 36, 28, 24, 22, 20][usize::from(name.as_bytes()[1] - b'1')];
                r.size = Some(HalfPoints(size));
            }
            _ => {}
        }
        if let Some(style) = attr_of(tag, "style") {
            apply_css(&style, &mut r);
        }
        let block = is_block(name);
        if block {
            self.flush();
            let align = attr_of(tag, "style")
                .and_then(|s| {
                    s.split(';').find_map(|d| {
                        let (k, v) = d.split_once(':')?;
                        (k.trim().eq_ignore_ascii_case("text-align"))
                            .then(|| v.trim().to_ascii_lowercase())
                    })
                })
                .or_else(|| attr_of(tag, "align"));
            self.para.align = match align.as_deref() {
                Some("center") => Some(Alignment::Center),
                Some("right") => Some(Alignment::Right),
                Some("justify") => Some(Alignment::Justify),
                _ => None,
            };
        }
        match name {
            "ul" | "ol" => self.lists.push((name == "ol", 0)),
            "li" => {
                let label = match self.lists.last_mut() {
                    Some((true, n)) => {
                        *n += 1;
                        format!("{n}. ")
                    }
                    _ => "• ".to_owned(),
                };
                self.atoms.push((r.clone(), RunContent::Text(label)));
            }
            _ => {}
        }
        self.stack.push((name.to_owned(), r));
    }

    fn close(&mut self, name: &str) {
        if let Some(i) = self.stack.iter().rposition(|(n, _)| n == name) {
            self.stack.truncate(i);
        }
        match name {
            "ul" | "ol" => {
                self.lists.pop();
            }
            "td" | "th" => {
                let props = self.props();
                self.atoms.push((props, RunContent::Tab));
                self.pending_space = false;
            }
            _ => {}
        }
        if is_block(name) {
            // Drop the trailing cell separator of a table row.
            if name == "tr" && matches!(self.atoms.last(), Some((_, RunContent::Tab))) {
                self.atoms.pop();
            }
            self.flush();
        }
    }
}

fn is_block(name: &str) -> bool {
    matches!(
        name,
        "p" | "div"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "li"
            | "tr"
            | "blockquote"
            | "pre"
            | "ul"
            | "ol"
            | "table"
            | "section"
            | "article"
            | "header"
            | "footer"
    )
}

/// Parse an HTML fragment into paragraphs.
pub fn from_html(html: &str) -> Vec<Block> {
    let mut imp = Importer {
        blocks: Vec::new(),
        atoms: Vec::new(),
        para: ParaProps::default(),
        stack: Vec::new(),
        lists: Vec::new(),
        skip: 0,
        pending_space: false,
    };
    let mut rest = html;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            if imp.skip == 0 {
                imp.text(rest);
            }
            break;
        };
        if lt > 0 && imp.skip == 0 {
            imp.text(&rest[..lt]);
        }
        rest = &rest[lt..];
        if let Some(body) = rest.strip_prefix("<!--") {
            rest = body.find("-->").map_or("", |e| &body[e + 3..]);
            continue;
        }
        let end = rest.find('>').unwrap_or(rest.len() - 1);
        let tag = &rest[1..end];
        rest = rest.get(end + 1..).unwrap_or("");
        let closing = tag.starts_with('/');
        let t = tag.trim_start_matches('/');
        let name: String = t
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect::<String>()
            .to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        let raw_content = matches!(name.as_str(), "script" | "style" | "head" | "title");
        if raw_content {
            if closing {
                imp.skip = imp.skip.saturating_sub(1);
            } else if !tag.ends_with('/') {
                imp.skip += 1;
            }
            continue;
        }
        if imp.skip > 0 {
            continue;
        }
        match (closing, name.as_str()) {
            (false, "br") => {
                let props = imp.props();
                imp.atoms.push((props, RunContent::LineBreak));
                imp.pending_space = false;
            }
            (false, n) => {
                let void = tag.ends_with('/')
                    || matches!(n, "img" | "meta" | "hr" | "input" | "link" | "col");
                if !void {
                    imp.open(n, t);
                }
            }
            (true, n) => imp.close(n),
        }
    }
    imp.flush();
    imp.blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_inline_formatting_and_paragraphs() {
        let html = r#"<html><head><style>p{}</style></head><body><!--StartFragment--><p style="text-align:center">Xin <b>chào</b> <span style="color:#ff0000;font-size:16px;font-style:italic">thế&nbsp;giới</span></p><p>Dòng 1<br>Dòng 2 &amp; <a href="https://example.com">liên kết</a></p><ul><li>một</li><li>hai</li></ul><table><tr><td>a</td><td>b</td></tr></table><!--EndFragment--></body></html>"#;
        let blocks = from_html(html);
        let paras: Vec<&Paragraph> = blocks.iter().filter_map(Block::as_paragraph).collect();
        let texts: Vec<String> = paras.iter().map(|p| p.plain_text()).collect();
        assert_eq!(
            texts,
            vec![
                "Xin chào thế\u{a0}giới",
                "Dòng 1\u{2028}Dòng 2 & liên kết",
                "• một",
                "• hai",
                "a\tb"
            ]
        );
        assert_eq!(paras[0].props.align, Some(Alignment::Center));
        let bold = &paras[0].runs[1];
        assert_eq!(bold.props.bold, Some(true));
        let red = paras[0].runs.last().unwrap();
        assert_eq!(red.props.color, Some(Color { r: 255, g: 0, b: 0 }));
        assert_eq!(red.props.size, Some(HalfPoints(24)));
        assert_eq!(red.props.italic, Some(true));
        assert_eq!(
            paras[1].runs.last().unwrap().props.link.as_deref(),
            Some("https://example.com")
        );
    }

    #[test]
    fn exports_html() {
        let doc = Document::default();
        let p = Paragraph::new(
            ParaProps {
                align: Some(Alignment::Right),
                ..Default::default()
            },
            vec![bo_model::Run::text(
                "a<b",
                RunProps {
                    bold: Some(true),
                    ..Default::default()
                },
            )],
        );
        let h = to_html(&doc, &[Block::Paragraph(p)]);
        assert!(h.contains("text-align:right"));
        assert!(h.contains("font-weight:bold"));
        assert!(h.contains("a&lt;b"));
        // Our own HTML imports back.
        let back = from_html(&h);
        assert_eq!(back[0].as_paragraph().unwrap().plain_text(), "a<b");
    }

    #[test]
    fn tolerates_garbage() {
        assert_eq!(from_html(""), Vec::<Block>::new());
        assert!(from_html("<<<>>>").len() <= 1);
        assert_eq!(
            from_html("plain &bogus; text")[0]
                .as_paragraph()
                .unwrap()
                .plain_text(),
            "plain &bogus; text"
        );
        assert_eq!(
            parse_color("#abc"),
            Some(Color {
                r: 0xAA,
                g: 0xBB,
                b: 0xCC
            })
        );
        assert_eq!(
            parse_color("rgb(1, 2, 3)"),
            Some(Color { r: 1, g: 2, b: 3 })
        );
    }
}
