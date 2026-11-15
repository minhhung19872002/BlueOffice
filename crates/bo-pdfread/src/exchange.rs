//! Comment and form data exchange for BlueEditor (Comment › Import / Export, Form › Import /
//! Export, Form to sheet): XFDF (ISO 19444-1, annotations and field values in XML), FDF
//! (ISO 32000-1 §12.7.8, field values) and CSV (field names in the first row, values in the
//! second). Imported comments become new annotations, imported values fill the fields; one
//! undo step.
use crate::annot::AnnotKind;
use crate::forms::FieldKind;
use crate::interp::apply;
use crate::{resolve, text_string, NewAnnot, PdfDocument};
use lopdf::Object;
use std::fmt::Write as _;

/// What an import brought in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportReport {
    /// Comments added.
    pub comments: usize,
    /// Field values set.
    pub fields: usize,
    /// Entries skipped (unknown kind, field or page).
    pub skipped: usize,
}

/// An XML element: name, attributes, children, text.
#[derive(Debug, Default, Clone)]
struct Node {
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<Node>,
    text: String,
}

impl Node {
    fn attr(&self, k: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.as_str())
    }

    fn child(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|c| c.name == name)
    }

    /// All text inside, children included.
    fn all_text(&self) -> String {
        let mut s = self.text.clone();
        for c in &self.children {
            s.push_str(&c.all_text());
        }
        s
    }
}

fn xml_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\r' => {}
            c if c.is_control() && c != '\n' && c != '\t' => {}
            c => o.push(c),
        }
    }
    o
}

fn xml_unescape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        o.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';').filter(|&e| e < 12) else {
            o.push('&');
            rest = &rest[1..];
            continue;
        };
        let ent = &rest[1..end];
        let ch = match ent {
            "lt" => Some('<'),
            "gt" => Some('>'),
            "amp" => Some('&'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            e if e.starts_with("#x") => u32::from_str_radix(&e[2..], 16)
                .ok()
                .and_then(char::from_u32),
            e if e.starts_with('#') => e[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        if let Some(c) = ch {
            o.push(c);
            rest = &rest[end + 1..];
        } else {
            o.push('&');
            rest = &rest[1..];
        }
    }
    o.push_str(rest);
    o
}

/// Parse XML into a tree under a nameless root (declarations, comments and CDATA markers
/// are skipped; enough for XFDF).
fn parse_xml(s: &str) -> Node {
    let mut stack: Vec<Node> = vec![Node::default()];
    let mut rest = s;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            if let Some(top) = stack.last_mut() {
                top.text.push_str(&xml_unescape(rest));
            }
            break;
        };
        if lt > 0 {
            if let Some(top) = stack.last_mut() {
                top.text.push_str(&xml_unescape(&rest[..lt]));
            }
        }
        rest = &rest[lt..];
        if let Some(r) = rest.strip_prefix("<!--") {
            rest = r.find("-->").map_or("", |e| &r[e + 3..]);
            continue;
        }
        if let Some(r) = rest.strip_prefix("<![CDATA[") {
            let e = r.find("]]>").unwrap_or(r.len());
            if let Some(top) = stack.last_mut() {
                top.text.push_str(&r[..e]);
            }
            rest = r.get(e + 3..).unwrap_or("");
            continue;
        }
        let Some(gt) = rest.find('>') else { break };
        let tag = &rest[1..gt];
        rest = &rest[gt + 1..];
        if tag.starts_with('?') || tag.starts_with('!') {
            continue;
        }
        if let Some(name) = tag.strip_prefix('/') {
            let name = name.trim();
            if stack.len() > 1 && stack.last().is_some_and(|n| n.name == name) {
                if let Some(done) = stack.pop() {
                    if let Some(top) = stack.last_mut() {
                        top.children.push(done);
                    }
                }
            }
            continue;
        }
        let empty = tag.ends_with('/');
        let tag = tag.trim_end_matches('/');
        let (name, mut attrs_src) = tag.split_once(char::is_whitespace).unwrap_or((tag, ""));
        let mut node = Node {
            name: name.to_owned(),
            ..Node::default()
        };
        while let Some(eq) = attrs_src.find('=') {
            let key = attrs_src[..eq].trim().to_owned();
            let after = attrs_src[eq + 1..].trim_start();
            let Some(q) = after.chars().next().filter(|c| *c == '"' || *c == '\'') else {
                break;
            };
            let body = &after[1..];
            let Some(end) = body.find(q) else { break };
            node.attrs.push((key, xml_unescape(&body[..end])));
            attrs_src = &body[end + 1..];
        }
        if empty {
            if let Some(top) = stack.last_mut() {
                top.children.push(node);
            }
        } else {
            stack.push(node);
        }
    }
    while stack.len() > 1 {
        if let Some(done) = stack.pop() {
            if let Some(top) = stack.last_mut() {
                top.children.push(done);
            }
        }
    }
    stack.pop().unwrap_or_default()
}

fn nums(s: &str) -> Vec<f32> {
    s.split([',', ' ', ';'])
        .filter_map(|t| t.trim().parse::<f32>().ok())
        .collect()
}

fn color_hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

fn parse_color(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// XFDF element of a comment kind, and back.
fn element(k: AnnotKind) -> Option<&'static str> {
    Some(match k {
        AnnotKind::Highlight => "highlight",
        AnnotKind::Underline => "underline",
        AnnotKind::StrikeOut => "strikeout",
        AnnotKind::Squiggly => "squiggly",
        AnnotKind::Note => "text",
        AnnotKind::TextBox => "freetext",
        AnnotKind::Square => "square",
        AnnotKind::Circle => "circle",
        _ => return None,
    })
}

fn kind_of(element: &str) -> Option<AnnotKind> {
    Some(match element {
        "highlight" => AnnotKind::Highlight,
        "underline" => AnnotKind::Underline,
        "strikeout" => AnnotKind::StrikeOut,
        "squiggly" => AnnotKind::Squiggly,
        "text" => AnnotKind::Note,
        "freetext" => AnnotKind::TextBox,
        "square" => AnnotKind::Square,
        "circle" => AnnotKind::Circle,
        _ => return None,
    })
}

/// A PDF literal string (§7.3.4.2) of `s` (UTF-16BE with a byte order mark when needed).
fn pdf_literal(s: &str) -> String {
    let bytes: Vec<u8> = if s.is_ascii() {
        s.as_bytes().to_vec()
    } else {
        let mut b = vec![0xfe, 0xff];
        b.extend(s.encode_utf16().flat_map(u16::to_be_bytes));
        b
    };
    let mut o = String::from("(");
    for b in bytes {
        match b {
            b'(' | b')' | b'\\' => {
                o.push('\\');
                o.push(char::from(b));
            }
            0x20..=0x7e => o.push(char::from(b)),
            _ => {
                let _ = write!(o, "\\{b:03o}");
            }
        }
    }
    o.push(')');
    o
}

/// Read a PDF literal string starting at `s` (after the opening parenthesis).
fn read_literal(s: &[u8]) -> (Vec<u8>, usize) {
    let mut out = Vec::new();
    let mut depth = 0;
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        match c {
            b'\\' if i + 1 < s.len() => {
                i += 1;
                let e = s[i];
                match e {
                    b'n' => out.push(b'\n'),
                    b'r' => out.push(b'\r'),
                    b't' => out.push(b'\t'),
                    b'b' => out.push(8),
                    b'f' => out.push(12),
                    b'0'..=b'7' => {
                        let mut v = 0u32;
                        let mut k = 0;
                        while k < 3 && i < s.len() && (b'0'..=b'7').contains(&s[i]) {
                            v = v * 8 + u32::from(s[i] - b'0');
                            i += 1;
                            k += 1;
                        }
                        out.push(v as u8);
                        continue;
                    }
                    b'\r' | b'\n' => {}
                    other => out.push(other),
                }
            }
            b'(' => {
                depth += 1;
                out.push(c);
            }
            b')' => {
                if depth == 0 {
                    return (out, i + 1);
                }
                depth -= 1;
                out.push(c);
            }
            _ => out.push(c),
        }
        i += 1;
    }
    (out, i)
}

/// Field name / value pairs of an FDF file (`/T (name) /V (value)` or `/V /Name`).
fn fdf_values(bytes: &[u8]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut i = 0;
    let mut name: Option<String> = None;
    while i + 2 < bytes.len() {
        if bytes[i] == b'/' && (bytes[i + 1] == b'T' || bytes[i + 1] == b'V') {
            let key = bytes[i + 1];
            let mut j = i + 2;
            if j < bytes.len() && bytes[j].is_ascii_alphanumeric() {
                i += 1;
                continue;
            }
            while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                j += 1;
            }
            let value = match bytes.get(j) {
                Some(b'(') => {
                    let (v, n) = read_literal(&bytes[j + 1..]);
                    j += 1 + n;
                    Some(text_string(&v))
                }
                Some(b'/') => {
                    let start = j + 1;
                    let mut e = start;
                    while e < bytes.len()
                        && !bytes[e].is_ascii_whitespace()
                        && !b"/<>[]()".contains(&bytes[e])
                    {
                        e += 1;
                    }
                    j = e;
                    Some(String::from_utf8_lossy(&bytes[start..e]).into_owned())
                }
                _ => None,
            };
            if let Some(v) = value {
                if key == b'T' {
                    name = Some(v);
                } else if let Some(n) = name.take() {
                    out.push((n, v));
                }
            }
            i = j;
            continue;
        }
        i += 1;
    }
    out
}

/// One CSV row (RFC 4180 quoting).
fn csv_row(cells: &[String]) -> String {
    cells
        .iter()
        .map(|c| {
            if c.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", c.replace('"', "\"\""))
            } else {
                c.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Rows of a CSV file.
fn csv_rows(s: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                cell.push('"');
                chars.next();
            }
            ('"', true) => quoted = false,
            ('"', false) if cell.is_empty() => quoted = true,
            (',', false) => row.push(std::mem::take(&mut cell)),
            ('\n', false) => {
                row.push(std::mem::take(&mut cell));
                rows.push(std::mem::take(&mut row));
            }
            ('\r', false) => {}
            (c, _) => cell.push(c),
        }
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    rows
}

impl PdfDocument {
    /// Field names and values, one per field (radio groups once), in field order.
    pub fn field_values(&self) -> Vec<(String, String)> {
        let mut out: Vec<(String, String)> = Vec::new();
        for f in self.form_fields() {
            if matches!(f.kind, FieldKind::Button | FieldKind::Signature) {
                continue;
            }
            if out.iter().any(|(n, _)| *n == f.name) {
                continue;
            }
            let v = match f.kind {
                FieldKind::CheckBox | FieldKind::Radio if f.value.is_empty() => "Off".to_owned(),
                _ => f.value.clone(),
            };
            out.push((f.name, v));
        }
        out
    }

    /// Field values as CSV: names in the first row, values in the second.
    pub fn fields_csv(&self) -> String {
        let v = self.field_values();
        let names: Vec<String> = v.iter().map(|x| x.0.clone()).collect();
        let values: Vec<String> = v.iter().map(|x| x.1.clone()).collect();
        format!("{}\r\n{}\r\n", csv_row(&names), csv_row(&values))
    }

    /// Field values as FDF (§12.7.8).
    pub fn fields_fdf(&self) -> Vec<u8> {
        let mut s = String::from("1 0 obj\n<< /FDF << /Fields [\n");
        for (n, v) in self.field_values() {
            let _ = writeln!(s, "<< /T {} /V {} >>", pdf_literal(&n), pdf_literal(&v));
        }
        s.push_str("] >> >>\nendobj\ntrailer\n<< /Root 1 0 R >>\n%%EOF\n");
        // A binary comment after the header line, as in a PDF (§7.5.2).
        let mut out = b"%FDF-1.2\n%\xe2\xe3\xcf\xd3\n".to_vec();
        out.extend_from_slice(s.as_bytes());
        out
    }

    /// XFDF of the comments (`comments`) and the field values (`fields`); coordinates in
    /// each page's default user space as the format asks.
    pub fn to_xfdf(&self, comments: bool, fields: bool, file: &str) -> String {
        let mut s = String::from(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<xfdf xmlns=\"http://ns.adobe.com/xfdf/\" xml:space=\"preserve\">\n",
        );
        if comments {
            s.push_str("<annots>\n");
            for a in self.annotations() {
                let Some(el) = element(a.kind) else { continue };
                let Some(d) = a.id.and_then(|id| self.doc.get_dictionary(id).ok()) else {
                    continue;
                };
                let arr = |k: &[u8]| -> Vec<f32> {
                    d.get(k)
                        .ok()
                        .and_then(|o| resolve(&self.doc, o).as_array().ok())
                        .map(|a| {
                            a.iter()
                                .filter_map(|o| resolve(&self.doc, o).as_float().ok())
                                .collect()
                        })
                        .unwrap_or_default()
                };
                let rect = arr(b"Rect");
                if rect.len() != 4 {
                    continue;
                }
                let join = |v: &[f32]| {
                    v.iter()
                        .map(|x| format!("{x:.2}"))
                        .collect::<Vec<_>>()
                        .join(",")
                };
                let _ = write!(
                    s,
                    "<{el} page=\"{}\" rect=\"{}\" color=\"{}\" title=\"{}\" date=\"{}\"",
                    a.page,
                    join(&rect),
                    color_hex(a.color),
                    xml_escape(&a.author),
                    xml_escape(&a.modified)
                );
                let quads = arr(b"QuadPoints");
                if !quads.is_empty() {
                    let _ = write!(s, " coords=\"{}\"", join(&quads));
                }
                if let Ok(ca) = d.get(b"CA").and_then(Object::as_float) {
                    let _ = write!(s, " opacity=\"{ca:.2}\"");
                }
                s.push('>');
                if !a.contents.is_empty() {
                    let _ = write!(s, "<contents>{}</contents>", xml_escape(&a.contents));
                }
                let _ = writeln!(s, "</{el}>");
            }
            s.push_str("</annots>\n");
        }
        if fields {
            s.push_str("<fields>\n");
            for (n, v) in self.field_values() {
                let _ = writeln!(
                    s,
                    "<field name=\"{}\"><value>{}</value></field>",
                    xml_escape(&n),
                    xml_escape(&v)
                );
            }
            s.push_str("</fields>\n");
        }
        let _ = writeln!(s, "<f href=\"{}\"/>\n</xfdf>", xml_escape(file));
        s
    }

    /// Set the fields named in `values`.
    fn fill_values(&mut self, values: &[(String, String)], rep: &mut ImportReport) {
        for (name, v) in values {
            let widgets: Vec<_> = self
                .form_fields()
                .into_iter()
                .filter(|f| f.name == *name)
                .collect();
            let Some(first) = widgets.first().cloned() else {
                rep.skipped += 1;
                continue;
            };
            let r = match first.kind {
                FieldKind::Text | FieldKind::Combo | FieldKind::List => {
                    self.set_field_text(&first, v)
                }
                FieldKind::CheckBox => {
                    let on = !(v.is_empty() || v == "Off" || v == "0" || v == "false");
                    self.set_field_checked(&first, on)
                }
                FieldKind::Radio => {
                    let pick = widgets
                        .iter()
                        .find(|w| w.on_state.as_deref() == Some(v.as_bytes()))
                        .cloned();
                    match pick {
                        Some(w) => self.set_field_checked(&w, true),
                        None => self.set_field_checked(&first, false),
                    }
                }
                FieldKind::Button | FieldKind::Signature => {
                    rep.skipped += 1;
                    continue;
                }
            };
            if r.is_ok() {
                rep.fields += 1;
            } else {
                rep.skipped += 1;
            }
        }
    }

    /// Add the comments of an XFDF `annots` element.
    fn add_xfdf_comments(&mut self, annots: &Node, author: &str, rep: &mut ImportReport) {
        for el in &annots.children {
            let Some(kind) = kind_of(&el.name) else {
                rep.skipped += 1;
                continue;
            };
            let page: usize = el.attr("page").and_then(|p| p.parse().ok()).unwrap_or(0);
            let Ok((base, _)) = self.bases(page) else {
                rep.skipped += 1;
                continue;
            };
            let to_page = |pts: &[f32]| -> Option<[f32; 4]> {
                let mut r = [
                    f32::INFINITY,
                    f32::INFINITY,
                    f32::NEG_INFINITY,
                    f32::NEG_INFINITY,
                ];
                for p in pts.chunks_exact(2) {
                    let (x, y) = apply(base, p[0], p[1]);
                    r = [r[0].min(x), r[1].min(y), r[2].max(x), r[3].max(y)];
                }
                r.iter().all(|v| v.is_finite()).then_some(r)
            };
            let rect = el.attr("rect").map(nums).unwrap_or_default();
            let quads = el.attr("coords").map(nums).unwrap_or_default();
            let mut rects: Vec<[f32; 4]> = quads.chunks_exact(8).filter_map(to_page).collect();
            if rects.is_empty() {
                if let Some(r) = (rect.len() == 4).then(|| to_page(&rect)).flatten() {
                    rects.push(r);
                }
            }
            if rects.is_empty() {
                rep.skipped += 1;
                continue;
            }
            let a = NewAnnot {
                kind,
                page,
                rects,
                color: el
                    .attr("color")
                    .and_then(parse_color)
                    .unwrap_or([255, 230, 0]),
                author: el
                    .attr("title")
                    .filter(|t| !t.is_empty())
                    .unwrap_or(author)
                    .to_owned(),
                contents: el
                    .child("contents")
                    .or_else(|| el.child("contents-richtext"))
                    .map(Node::all_text)
                    .unwrap_or_default(),
            };
            if self.add_annotation(&a).is_ok() {
                rep.comments += 1;
            } else {
                rep.skipped += 1;
            }
        }
    }

    /// Import comments and / or field values from an XFDF, FDF or CSV file (one undo step).
    pub fn import_data(&mut self, bytes: &[u8], author: &str) -> Result<ImportReport, String> {
        let mut rep = ImportReport::default();
        let head: Vec<u8> = bytes.iter().take(1024).copied().collect();
        let text = String::from_utf8_lossy(bytes);
        self.begin_group("import");
        if head.windows(4).any(|w| w == b"%FDF") {
            let values = fdf_values(bytes);
            self.fill_values(&values, &mut rep);
        } else if text.contains("<xfdf") {
            let root = parse_xml(&text);
            let x = root.child("xfdf").cloned().unwrap_or(root);
            if let Some(a) = x.child("annots") {
                self.add_xfdf_comments(a, author, &mut rep);
            }
            if let Some(f) = x.child("fields") {
                let mut values = Vec::new();
                fn walk(n: &Node, prefix: &str, out: &mut Vec<(String, String)>) {
                    for c in n.children.iter().filter(|c| c.name == "field") {
                        let name = c.attr("name").unwrap_or_default();
                        let full = if prefix.is_empty() {
                            name.to_owned()
                        } else {
                            format!("{prefix}.{name}")
                        };
                        if let Some(v) = c.child("value") {
                            out.push((full.clone(), v.all_text()));
                        }
                        walk(c, &full, out);
                    }
                }
                walk(f, "", &mut values);
                self.fill_values(&values, &mut rep);
            }
        } else {
            let rows = csv_rows(text.trim_start_matches('\u{feff}'));
            if let (Some(names), Some(vals)) = (rows.first(), rows.get(1)) {
                let values: Vec<(String, String)> = names
                    .iter()
                    .zip(vals)
                    .map(|(n, v)| (n.trim().to_owned(), v.clone()))
                    .collect();
                self.fill_values(&values, &mut rep);
            }
        }
        self.end_group();
        if rep.comments == 0 && rep.fields == 0 && rep.skipped == 0 {
            return Err("nothing to import".into());
        }
        Ok(rep)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_and_csv_and_fdf_parsers() {
        let x = parse_xml(
            "<?xml version=\"1.0\"?><xfdf><annots><text page=\"1\" rect=\"1,2,3,4\">\
             <contents>a &amp; b</contents></text></annots><f href=\"x\"/></xfdf>",
        );
        let xfdf = x.child("xfdf").expect("xfdf");
        let t = &xfdf.child("annots").expect("annots").children[0];
        assert_eq!(t.attr("page"), Some("1"));
        assert_eq!(
            nums(t.attr("rect").expect("rect")),
            vec![1.0, 2.0, 3.0, 4.0]
        );
        assert_eq!(
            t.child("contents").map(Node::all_text).as_deref(),
            Some("a & b")
        );
        assert_eq!(xfdf.child("f").and_then(|f| f.attr("href")), Some("x"));
        let rows = csv_rows("a,\"b,c\",\"d\"\"e\"\r\n1,2,3\r\n");
        assert_eq!(rows, vec![vec!["a", "b,c", "d\"e"], vec!["1", "2", "3"]]);
        assert_eq!(csv_row(&["x,y".into(), "z".into()]), "\"x,y\",z");
        let f = fdf_values(
            b"%FDF-1.2 << /Fields [<< /T (name) /V (J\\(o\\)hn) >> << /V /Yes /T (c) >>] >>",
        );
        assert_eq!(f, vec![("name".to_owned(), "J(o)hn".to_owned())]);
        assert_eq!(pdf_literal("a(b)"), "(a\\(b\\))");
    }
}
