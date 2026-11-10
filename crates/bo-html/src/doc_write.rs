//! Word-processing document → one "filtered" HTML file.
//!
//! Plain HTML and CSS only (no office-specific markup): the document's styles become
//! classes of a `<style>` sheet, direct formatting becomes `style` attributes,
//! headings `h1`–`h6`, numbered and bulleted paragraphs `ol` / `ul`, tables `table`,
//! links `a`. Pictures are embedded as `data:` URLs (RFC 2397) or handed to the
//! caller as separate resources (MHTML). Footnotes and endnotes are gathered at the
//! end of the page, linked from their references.

use crate::entities::{escape, escape_attr};
use crate::image;
use base64::Engine as _;
use bo_model::{
    Alignment, Block, Border, Borders, CellVAlign, Color, Document, LineSpacing, NumFormat, NumRef,
    ParaProps, Paragraph, Run, RunContent, RunProps, StyleKind, Table, TableWidth, Underline,
    VMerge, VertAlign,
};
use std::collections::BTreeSet;
use std::fmt::Write as _;

/// A picture written next to the page instead of inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    /// URL the page uses (`cid:…`).
    pub url: String,
    /// File name (`image1.png`).
    pub name: String,
    /// Media type.
    pub mime: String,
    /// Bytes.
    pub data: Vec<u8>,
}

/// Where pictures go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Images {
    /// Inside the page, as `data:` URLs.
    #[default]
    DataUrl,
    /// Beside the page, referenced by `cid:` URLs (RFC 2392) — MHTML.
    Cid,
}

/// Options of [`write_html_with`].
#[derive(Debug, Clone, Default)]
pub struct WriteOptions {
    /// Page title (`<title>`).
    pub title: String,
    /// Where pictures go.
    pub images: Images,
}

/// The document as one HTML file with embedded pictures.
pub fn write_html(doc: &Document, title: &str) -> String {
    write_html_with(
        doc,
        &WriteOptions {
            title: title.to_owned(),
            images: Images::DataUrl,
        },
    )
    .0
}

/// The document as HTML plus the pictures it refers to (empty for [`Images::DataUrl`]).
pub fn write_html_with(doc: &Document, opts: &WriteOptions) -> (String, Vec<Resource>) {
    let mut w = Writer {
        doc,
        images: opts.images,
        resources: Vec::new(),
        out: String::new(),
        used_para: BTreeSet::new(),
        used_char: BTreeSet::new(),
        notes: Vec::new(),
        depth: 0,
    };
    for s in &doc.sections {
        w.blocks(&s.blocks, None);
    }
    w.notes_section();
    let body = std::mem::take(&mut w.out);
    let mut html = String::with_capacity(body.len() + 2048);
    html.push_str("<!DOCTYPE html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"generator\" content=\"BlueOffice\">\n");
    let _ = writeln!(html, "<title>{}</title>", escape(&opts.title));
    html.push_str("<style>\n");
    html.push_str(&w.style_sheet());
    html.push_str("</style>\n</head>\n<body>\n");
    html.push_str(&body);
    html.push_str("</body>\n</html>\n");
    (html, w.resources)
}

fn pt(twips: i32) -> String {
    num(f64::from(twips) / 20.0)
}

/// A number with at most four decimals and no trailing zeros.
fn num(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".into()
    } else {
        s.to_owned()
    }
}

fn hex(c: Color) -> String {
    format!("#{}", c.to_hex())
}

/// CSS class of a style id.
fn class_name(prefix: &str, id: &str) -> String {
    let clean: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!("{prefix}{clean}")
}

fn border_css(b: &Border) -> String {
    if !b.is_visible() {
        return "none".into();
    }
    let style = match b.style.as_str() {
        "double" | "triple" => "double",
        "dashed" | "dashSmallGap" | "dotDash" | "dotDotDash" => "dashed",
        "dotted" => "dotted",
        _ => "solid",
    };
    let width = (f64::from(b.size) / 8.0).max(0.25);
    // A double line needs three line widths to show.
    let width = if style == "double" {
        width.max(2.25)
    } else {
        width
    };
    format!(
        "{}pt {style} {}",
        num(width),
        b.color.map_or_else(|| "#000000".to_owned(), hex)
    )
}

/// `border…` declarations of four edges (top, left, bottom, right).
fn borders_css(edges: [Option<&Border>; 4], css: &mut String) {
    if edges.iter().all(Option::is_none) {
        return;
    }
    let all: Vec<String> = edges
        .iter()
        .map(|e| e.map_or_else(|| "none".to_owned(), border_css))
        .collect();
    if all.iter().all(|b| *b == all[0]) {
        let _ = write!(css, "border:{};", all[0]);
        return;
    }
    for (side, b) in ["top", "left", "bottom", "right"].iter().zip(&all) {
        if b != "none" {
            let _ = write!(css, "border-{side}:{b};");
        }
    }
}

/// Paragraph properties as CSS declarations. `list`: indents come from the list.
fn para_css(p: &ParaProps, list: bool, css: &mut String) {
    if let Some(a) = p.align {
        let v = match a {
            Alignment::Left => "left",
            Alignment::Center => "center",
            Alignment::Right => "right",
            Alignment::Justify => "justify",
        };
        let _ = write!(css, "text-align:{v};");
    }
    if !list {
        if let Some(t) = p.indent_left {
            let _ = write!(css, "margin-left:{}pt;", pt(t.0));
        }
        if let Some(t) = p.indent_first_line {
            let _ = write!(css, "text-indent:{}pt;", pt(t.0));
        }
    }
    if let Some(t) = p.indent_right {
        let _ = write!(css, "margin-right:{}pt;", pt(t.0));
    }
    if let Some(t) = p.space_before {
        let _ = write!(css, "margin-top:{}pt;", pt(t.0));
    }
    if let Some(t) = p.space_after {
        let _ = write!(css, "margin-bottom:{}pt;", pt(t.0));
    }
    match p.line_spacing {
        Some(LineSpacing::Auto(n)) => {
            let k = f64::from(n) / 240.0 * crate::doc_read::LINE_FACTOR;
            let _ = write!(css, "line-height:{};", num(k));
        }
        Some(LineSpacing::Exact(t) | LineSpacing::AtLeast(t)) => {
            let _ = write!(css, "line-height:{}pt;", pt(t.0));
        }
        None => {}
    }
    if let Some(c) = p.shading {
        let _ = write!(css, "background-color:{};", hex(c));
    }
    if let Some(b) = &p.borders {
        borders_css(
            [
                b.top.as_ref(),
                b.left.as_ref(),
                b.bottom.as_ref(),
                b.right.as_ref(),
            ],
            css,
        );
    }
    if p.page_break_before == Some(true) {
        css.push_str("page-break-before:always;");
    }
    if p.keep_next == Some(true) {
        css.push_str("page-break-after:avoid;");
    }
}

/// Run properties as CSS declarations (superscript / subscript are elements).
fn run_css(r: &RunProps, css: &mut String) {
    if let Some(f) = r.font.as_ref().or(r.font_h_ansi.as_ref()) {
        let safe: String = f
            .chars()
            .filter(|c| !matches!(c, '\'' | '"' | '<' | '>' | ';' | '\\'))
            .collect();
        let _ = write!(css, "font-family:'{safe}';");
    }
    if let Some(s) = r.size {
        let _ = write!(css, "font-size:{}pt;", num(f64::from(s.0) / 2.0));
    }
    if let Some(b) = r.bold {
        let _ = write!(css, "font-weight:{};", if b { "bold" } else { "normal" });
    }
    if let Some(i) = r.italic {
        let _ = write!(css, "font-style:{};", if i { "italic" } else { "normal" });
    }
    if r.underline.is_some() || r.strike.is_some() || r.double_strike.is_some() {
        let mut parts = Vec::new();
        match r.underline {
            Some(Underline::None) | None => {}
            Some(Underline::Double) => parts.push("underline double"),
            Some(Underline::Dotted) => parts.push("underline dotted"),
            Some(_) => parts.push("underline"),
        }
        if r.strike == Some(true) || r.double_strike == Some(true) {
            parts.push("line-through");
        }
        let _ = write!(
            css,
            "text-decoration:{};",
            if parts.is_empty() {
                "none".to_owned()
            } else {
                parts.join(" ")
            }
        );
    }
    if let Some(c) = r.color {
        let _ = write!(css, "color:{};", hex(c));
    }
    if let Some(c) = r.highlight.or(r.shading) {
        let _ = write!(css, "background-color:{};", hex(c));
    }
    if let Some(c) = r.caps {
        let _ = write!(
            css,
            "text-transform:{};",
            if c { "uppercase" } else { "none" }
        );
    }
    if let Some(c) = r.small_caps {
        let _ = write!(
            css,
            "font-variant:{};",
            if c { "small-caps" } else { "normal" }
        );
    }
    if let Some(s) = r.spacing {
        let _ = write!(css, "letter-spacing:{}pt;", pt(s.0));
    }
}

/// White space that HTML would collapse: the paragraph needs `white-space: pre-wrap`.
fn needs_pre(text: &str) -> bool {
    text.starts_with(' ') || text.ends_with(' ') || text.contains("  ") || text.contains('\t')
}

/// A note gathered for the end of the page.
struct Note<'a> {
    key: String,
    label: String,
    endnote: bool,
    blocks: &'a [Block],
}

/// An open list: element name and level.
struct OpenList {
    tag: &'static str,
    ilvl: u8,
    num_id: i32,
    /// An `li` is open.
    item_open: bool,
}

struct Writer<'a> {
    doc: &'a Document,
    images: Images,
    resources: Vec<Resource>,
    out: String,
    used_para: BTreeSet<String>,
    used_char: BTreeSet<String>,
    notes: Vec<Note<'a>>,
    /// Nesting of tables / text boxes / notes (bounded).
    depth: usize,
}

/// Deepest nesting of tables and text boxes written.
const MAX_NESTING: usize = 32;

impl<'a> Writer<'a> {
    /// The `<style>` sheet: page defaults and one class per style in use.
    fn style_sheet(&self) -> String {
        let mut s = String::new();
        let mut body = String::new();
        run_css(&self.doc.default_run, &mut body);
        if let Some(sec) = self.doc.sections.first() {
            let p = &sec.page;
            let _ = writeln!(
                s,
                "@page{{size:{}pt {}pt;margin:{}pt {}pt {}pt {}pt}}",
                pt(p.width.0),
                pt(p.height.0),
                pt(p.margin_top.0),
                pt(p.margin_right.0),
                pt(p.margin_bottom.0),
                pt(p.margin_left.0)
            );
        }
        let _ = writeln!(s, "body{{{body}}}");
        let mut para = String::from("margin:0;");
        para_css(&self.doc.default_para, false, &mut para);
        let _ = writeln!(
            s,
            "p,h1,h2,h3,h4,h5,h6,li{{{para}font-size:inherit;font-weight:inherit}}"
        );
        s.push_str("ul,ol{margin-top:0;margin-bottom:0}\n");
        s.push_str("table{border-collapse:collapse}\ntd,th{padding:0 5.4pt;vertical-align:top}\n");
        s.push_str("a.bo-noteref,a.bo-notemark{text-decoration:none}\n");
        for id in &self.used_para {
            let (pp, rp) = self.doc.styles.resolve_paragraph(id);
            let mut css = String::new();
            para_css(&pp, false, &mut css);
            run_css(&rp, &mut css);
            if !css.is_empty() {
                let _ = writeln!(s, ".{}{{{css}}}", class_name("s-", id));
            }
        }
        for id in &self.used_char {
            let mut css = String::new();
            run_css(&self.doc.styles.resolve_character(id), &mut css);
            if !css.is_empty() {
                let _ = writeln!(s, ".{}{{{css}}}", class_name("c-", id));
            }
        }
        s
    }

    /// Style id of a paragraph: its own, else the default paragraph style.
    fn para_style(&self, p: &Paragraph) -> Option<&'a str> {
        match &p.props.style {
            Some(id) => self.doc.style(id).map(|s| s.id.as_str()),
            None => self
                .doc
                .styles
                .default_of(StyleKind::Paragraph)
                .map(|s| s.id.as_str()),
        }
    }

    /// List membership of a paragraph: numbering reference and its level definition.
    fn list_of(&self, p: &Paragraph) -> Option<(NumRef, &'a bo_model::Level)> {
        let style_num = || {
            let id = self.para_style(p)?;
            let (pp, _) = self.doc.styles.resolve_paragraph(id);
            pp.num.map(|n| self.doc.numbering.level_for_style(n, id))
        };
        let r = p.props.num.or_else(style_num)?;
        if r.num_id == 0 {
            return None;
        }
        let level = self.doc.numbering.level(r)?;
        (!matches!(level.format, NumFormat::None)).then_some((r, level))
    }

    fn close_lists(&mut self, lists: &mut Vec<OpenList>, keep: usize) {
        while lists.len() > keep {
            if let Some(l) = lists.pop() {
                if l.item_open {
                    self.out.push_str("</li>\n");
                }
                let _ = writeln!(self.out, "</{}>", l.tag);
            }
        }
    }

    /// Blocks of a container. `note`: key of the note being written (for its mark).
    fn blocks(&mut self, blocks: &'a [Block], note: Option<&str>) {
        let mut lists: Vec<OpenList> = Vec::new();
        for b in blocks {
            match b {
                Block::Paragraph(p) => {
                    if let Some((r, level)) = self.list_of(p) {
                        self.list_item(p, r, level, &mut lists, note);
                    } else {
                        self.close_lists(&mut lists, 0);
                        self.paragraph(p, note);
                    }
                }
                Block::Table(t) => {
                    self.close_lists(&mut lists, 0);
                    self.table(t, note);
                }
                Block::Opaque(_) => {}
            }
        }
        self.close_lists(&mut lists, 0);
    }

    fn list_item(
        &mut self,
        p: &'a Paragraph,
        r: NumRef,
        level: &bo_model::Level,
        lists: &mut Vec<OpenList>,
        note: Option<&str>,
    ) {
        let bullet = matches!(level.format, NumFormat::Bullet);
        let tag = if bullet { "ul" } else { "ol" };
        // Close deeper lists, and a list of another kind or instance at this level.
        while let Some(top) = lists.last() {
            let deeper = top.ilvl > r.ilvl;
            let other = top.ilvl == r.ilvl && (top.tag != tag || top.num_id != r.num_id);
            if deeper || other {
                let keep = lists.len() - 1;
                self.close_lists(lists, keep);
            } else {
                break;
            }
        }
        match lists.last_mut() {
            Some(top) if top.ilvl == r.ilvl => {
                if top.item_open {
                    self.out.push_str("</li>\n");
                }
            }
            _ => {
                // A new (nested) list; it sits inside the open item of its parent.
                let mut open = format!("<{tag}");
                if !bullet {
                    let ty = match level.format {
                        NumFormat::LowerLetter => Some("a"),
                        NumFormat::UpperLetter => Some("A"),
                        NumFormat::LowerRoman => Some("i"),
                        NumFormat::UpperRoman => Some("I"),
                        _ => None,
                    };
                    if let Some(ty) = ty {
                        let _ = write!(open, " type=\"{ty}\"");
                    }
                    let start = self.doc.numbering.start(r);
                    if start != 1 {
                        let _ = write!(open, " start=\"{start}\"");
                    }
                }
                let _ = writeln!(self.out, "{open}>");
                lists.push(OpenList {
                    tag,
                    ilvl: r.ilvl,
                    num_id: r.num_id,
                    item_open: false,
                });
            }
        }
        let (class, css) = self.para_attrs(p, true);
        let _ = write!(self.out, "<li{class}{css}>");
        self.inline(p, note);
        if let Some(top) = lists.last_mut() {
            top.item_open = true;
        }
    }

    /// ` class="…"` and ` style="…"` of a paragraph element.
    fn para_attrs(&mut self, p: &Paragraph, list: bool) -> (String, String) {
        let class = match self.para_style(p) {
            Some(id) => {
                self.used_para.insert(id.to_owned());
                format!(" class=\"{}\"", class_name("s-", id))
            }
            None => String::new(),
        };
        let mut css = String::new();
        para_css(&p.props, list, &mut css);
        if needs_pre(&p.plain_text()) {
            css.push_str("white-space:pre-wrap;");
        }
        let css = if css.is_empty() {
            String::new()
        } else {
            format!(" style=\"{}\"", escape_attr(&css))
        };
        (class, css)
    }

    fn paragraph(&mut self, p: &'a Paragraph, note: Option<&str>) {
        // Heading level: the outline level of the paragraph or of its style.
        let outline = p.props.outline_level.or_else(|| {
            self.para_style(p)
                .and_then(|id| self.doc.styles.resolve_paragraph(id).0.outline_level)
        });
        let tag = match outline {
            Some(l @ 0..=5) => ["h1", "h2", "h3", "h4", "h5", "h6"][usize::from(l)],
            _ => "p",
        };
        let (class, css) = self.para_attrs(p, false);
        let _ = write!(self.out, "<{tag}{class}{css}>");
        self.inline(p, note);
        let _ = writeln!(self.out, "</{tag}>");
    }

    /// Runs of a paragraph, then the text boxes anchored in it.
    fn inline(&mut self, p: &'a Paragraph, note: Option<&str>) {
        let start = self.out.len();
        let mut boxes: Vec<&'a [Block]> = Vec::new();
        let mut open_link: Option<&str> = None;
        let mut last_break = false;
        for run in &p.runs {
            // Tracked deletions and hidden text are not part of the page.
            if run.props.del.is_some() || run.props.hidden == Some(true) {
                continue;
            }
            let link = run.props.link.as_deref();
            if link != open_link {
                if open_link.is_some() {
                    self.out.push_str("</a>");
                }
                if let Some(href) = link {
                    let _ = write!(self.out, "<a href=\"{}\">", escape_attr(href));
                }
                open_link = link;
            }
            self.run(run, note, &mut boxes, &mut last_break);
        }
        if open_link.is_some() {
            self.out.push_str("</a>");
        }
        // An empty paragraph keeps its line; a final line break needs a second one to show.
        if self.out.len() == start || last_break {
            self.out.push_str("<br>");
        }
        if !boxes.is_empty() && self.depth < MAX_NESTING {
            self.depth += 1;
            for b in boxes {
                self.out
                    .push_str("<div class=\"bo-textbox\" style=\"border:0.75pt solid #808080;padding:3.6pt 7.2pt;margin:6pt 0\">\n");
                self.blocks(b, None);
                self.out.push_str("</div>");
            }
            self.depth -= 1;
        }
    }

    fn run(
        &mut self,
        run: &'a Run,
        note: Option<&str>,
        boxes: &mut Vec<&'a [Block]>,
        last_break: &mut bool,
    ) {
        let mut css = String::new();
        run_css(&run.props, &mut css);
        let class = run
            .props
            .style
            .as_deref()
            .and_then(|id| self.doc.style(id))
            .map(|s| {
                self.used_char.insert(s.id.clone());
                format!(" class=\"{}\"", class_name("c-", &s.id))
            })
            .unwrap_or_default();
        let (open, close) = {
            let mut open = String::new();
            let mut close = String::new();
            if !css.is_empty() || !class.is_empty() {
                let style = if css.is_empty() {
                    String::new()
                } else {
                    format!(" style=\"{}\"", escape_attr(&css))
                };
                let _ = write!(open, "<span{class}{style}>");
                close.push_str("</span>");
            }
            match run.props.vert_align {
                Some(VertAlign::Superscript) => {
                    open.push_str("<sup>");
                    close.insert_str(0, "</sup>");
                }
                Some(VertAlign::Subscript) => {
                    open.push_str("<sub>");
                    close.insert_str(0, "</sub>");
                }
                _ => {}
            }
            (open, close)
        };
        let mut body = String::new();
        for c in &run.content {
            let before = body.len();
            match c {
                RunContent::Text(t) => body.push_str(&escape(t)),
                RunContent::Tab => body.push('\t'),
                RunContent::LineBreak | RunContent::ColumnBreak => body.push_str("<br>"),
                RunContent::PageBreak => body.push_str("<br style=\"page-break-before:always\">"),
                RunContent::NoBreakHyphen => body.push('\u{2011}'),
                RunContent::SoftHyphen => body.push_str("&shy;"),
                RunContent::Field { cached, .. } => body.push_str(&escape(cached)),
                RunContent::Image(img) => {
                    if let Some(shape) = &img.shape {
                        if let Some(text) = &shape.text {
                            boxes.push(text);
                        }
                    } else {
                        self.image(img, &mut body);
                    }
                }
                RunContent::FootnoteRef(id) | RunContent::EndnoteRef(id) => {
                    let endnote = matches!(c, RunContent::EndnoteRef(_));
                    let map = if endnote {
                        &self.doc.endnotes
                    } else {
                        &self.doc.footnotes
                    };
                    if let Some(blocks) = map.get(id) {
                        let n =
                            self.notes.iter().filter(|x| x.endnote == endnote).count() as i32 + 1;
                        let label = if endnote {
                            bo_model::format_number(n, &NumFormat::LowerRoman)
                        } else {
                            n.to_string()
                        };
                        let key = format!(
                            "{}{}",
                            if endnote { "e" } else { "f" },
                            self.notes.len() + 1
                        );
                        // The superscript comes from the run itself when it has one.
                        let (so, sc) = if run.props.vert_align == Some(VertAlign::Superscript) {
                            ("", "")
                        } else {
                            ("<sup>", "</sup>")
                        };
                        let _ = write!(
                            body,
                            "<a class=\"bo-noteref\" id=\"bo-ref-{key}\" href=\"#bo-note-{key}\">{so}{label}{sc}</a>"
                        );
                        self.notes.push(Note {
                            key,
                            label,
                            endnote,
                            blocks,
                        });
                    }
                }
                RunContent::NoteMark => {
                    if let Some(key) = note {
                        let label = self
                            .notes
                            .iter()
                            .find(|n| n.key == key)
                            .map(|n| n.label.clone())
                            .unwrap_or_default();
                        let (so, sc) = if run.props.vert_align == Some(VertAlign::Superscript) {
                            ("", "")
                        } else {
                            ("<sup>", "</sup>")
                        };
                        let _ = write!(
                            body,
                            "<a class=\"bo-notemark\" href=\"#bo-ref-{key}\">{so}{label}{sc}</a>"
                        );
                    }
                }
                _ => {}
            }
            if body.len() > before {
                *last_break = matches!(c, RunContent::LineBreak | RunContent::ColumnBreak);
            }
        }
        if !body.is_empty() {
            self.out.push_str(&open);
            self.out.push_str(&body);
            self.out.push_str(&close);
        }
    }

    fn image(&mut self, img: &bo_model::Image, body: &mut String) {
        let Some(data) = self.doc.media.get(&img.media) else {
            body.push_str(&escape(&img.description));
            return;
        };
        let Some(info) = image::sniff(data) else {
            // A format browsers do not show (EMF, WMF, TIFF, …): its description.
            body.push_str(&escape(&img.description));
            return;
        };
        let src = match self.images {
            Images::DataUrl => format!(
                "data:{};base64,{}",
                info.mime,
                base64::engine::general_purpose::STANDARD.encode(data.as_slice())
            ),
            Images::Cid => {
                let existing = self
                    .resources
                    .iter()
                    .find(|r| r.data.as_slice() == data.as_slice())
                    .map(|r| r.url.clone());
                existing.unwrap_or_else(|| {
                    let name = format!("image{}.{}", self.resources.len() + 1, info.ext);
                    let url = format!("cid:{name}@blueoffice");
                    self.resources.push(Resource {
                        url: url.clone(),
                        name,
                        mime: info.mime.to_owned(),
                        data: data.as_ref().clone(),
                    });
                    url
                })
            }
        };
        let (w, h) = (
            img.width.0 as f64 / 12_700.0,
            img.height.0 as f64 / 12_700.0,
        );
        let _ = write!(
            body,
            "<img src=\"{}\" width=\"{}\" height=\"{}\" style=\"width:{}pt;height:{}pt\" alt=\"{}\">",
            escape_attr(&src),
            (w / 0.75).round().max(1.0),
            (h / 0.75).round().max(1.0),
            num(w),
            num(h),
            escape_attr(&img.description)
        );
    }

    #[allow(clippy::too_many_lines)]
    fn table(&mut self, t: &'a Table, note: Option<&str>) {
        if self.depth >= MAX_NESTING {
            return;
        }
        self.depth += 1;
        // Borders: the table's own, else those of its table style.
        let style_borders = t.props.style.as_deref().and_then(|id| {
            self.doc
                .styles
                .chain(id)
                .into_iter()
                .rev()
                .find_map(|s| s.tblpr.as_ref().and_then(|p| p.borders.clone()))
        });
        let tb: Borders = t
            .props
            .borders
            .clone()
            .or(style_borders)
            .unwrap_or_default();
        let mut css = String::new();
        match t.props.width {
            Some(TableWidth::Dxa(w)) if w.0 > 0 => {
                let _ = write!(css, "width:{}pt;", pt(w.0));
            }
            Some(TableWidth::Pct(p)) if p > 0 => {
                let _ = write!(css, "width:{}%;", num(f64::from(p) / 50.0));
            }
            _ => {}
        }
        match t.props.align {
            Some(Alignment::Center) => css.push_str("margin-left:auto;margin-right:auto;"),
            Some(Alignment::Right) => css.push_str("margin-left:auto;"),
            _ => {
                if let Some(i) = t.props.indent {
                    let _ = write!(css, "margin-left:{}pt;", pt(i.0));
                }
            }
        }
        let _ = writeln!(self.out, "<table style=\"{}\">", escape_attr(&css));
        if !t.grid.is_empty() {
            self.out.push_str("<colgroup>");
            for g in &t.grid {
                let _ = write!(self.out, "<col style=\"width:{}pt\">", pt(g.0));
            }
            self.out.push_str("</colgroup>\n");
        }
        let n_rows = t.rows.len();
        let n_cols = t
            .rows
            .iter()
            .map(|r| {
                r.cells
                    .iter()
                    .map(|c| c.props.grid_span.max(1) as usize)
                    .sum::<usize>()
            })
            .max()
            .unwrap_or(0);
        // Leading header rows form the table head.
        let head_rows = t.rows.iter().take_while(|r| r.props.header).count();
        if head_rows > 0 {
            self.out.push_str("<thead>\n");
        }
        for (ri, row) in t.rows.iter().enumerate() {
            if ri == head_rows && head_rows > 0 {
                self.out.push_str("</thead>\n");
            }
            if row.props.del.is_some() {
                continue;
            }
            let mut tr = String::from("<tr");
            if let Some((h, _)) = row.props.height {
                let _ = write!(tr, " style=\"height:{}pt\"", pt(h.0));
            }
            let _ = write!(self.out, "{tr}>");
            let mut col = 0usize;
            for cell in &row.cells {
                let span = cell.props.grid_span.max(1) as usize;
                let at = col;
                col += span;
                if cell.props.v_merge == Some(VMerge::Continue) && ri > 0 {
                    continue;
                }
                // Rows merged below: cells continuing the merge at the same grid column.
                let mut rowspan = 1;
                if cell.props.v_merge == Some(VMerge::Restart) {
                    for below in &t.rows[ri + 1..] {
                        let mut c = 0usize;
                        let cont = below.cells.iter().any(|x| {
                            let here = c;
                            c += x.props.grid_span.max(1) as usize;
                            here == at && x.props.v_merge == Some(VMerge::Continue)
                        });
                        if !cont {
                            break;
                        }
                        rowspan += 1;
                    }
                }
                let header = ri < head_rows;
                let tag = if header { "th" } else { "td" };
                let mut open = format!("<{tag}");
                if span > 1 {
                    let _ = write!(open, " colspan=\"{span}\"");
                }
                if rowspan > 1 {
                    let _ = write!(open, " rowspan=\"{rowspan}\"");
                }
                let mut css = String::new();
                let cb = cell.props.borders.as_ref();
                let edges = [
                    pick(
                        cb.and_then(|b| b.top.as_ref()),
                        ri == 0,
                        tb.top.as_ref(),
                        tb.inside_h.as_ref(),
                    ),
                    pick(
                        cb.and_then(|b| b.left.as_ref()),
                        at == 0,
                        tb.left.as_ref(),
                        tb.inside_v.as_ref(),
                    ),
                    pick(
                        cb.and_then(|b| b.bottom.as_ref()),
                        ri + rowspan >= n_rows,
                        tb.bottom.as_ref(),
                        tb.inside_h.as_ref(),
                    ),
                    pick(
                        cb.and_then(|b| b.right.as_ref()),
                        at + span >= n_cols,
                        tb.right.as_ref(),
                        tb.inside_v.as_ref(),
                    ),
                ];
                borders_css(edges, &mut css);
                if let Some(c) = cell.props.shading {
                    let _ = write!(css, "background-color:{};", hex(c));
                }
                match cell.props.v_align {
                    Some(CellVAlign::Center) => css.push_str("vertical-align:middle;"),
                    Some(CellVAlign::Bottom) => css.push_str("vertical-align:bottom;"),
                    Some(CellVAlign::Top) | None => {}
                }
                if header {
                    css.push_str("font-weight:normal;text-align:left;");
                }
                if !css.is_empty() {
                    let _ = write!(open, " style=\"{}\"", escape_attr(&css));
                }
                let _ = writeln!(self.out, "{open}>");
                self.blocks(&cell.blocks, note);
                let _ = write!(self.out, "</{tag}>");
            }
            self.out.push_str("</tr>\n");
        }
        if head_rows > 0 && head_rows == t.rows.len() {
            self.out.push_str("</thead>\n");
        }
        self.out.push_str("</table>\n");
        self.depth -= 1;
    }

    /// Footnotes and endnotes, in the order of their references. Notes referenced
    /// from notes are appended as they are met.
    fn notes_section(&mut self) {
        if self.notes.is_empty() {
            return;
        }
        self.out
            .push_str("<hr class=\"bo-notes-sep\">\n<div class=\"bo-notes\">\n");
        let mut i = 0;
        while i < self.notes.len() && i < 100_000 {
            let key = self.notes[i].key.clone();
            let blocks = self.notes[i].blocks;
            let kind = if self.notes[i].endnote {
                "endnote"
            } else {
                "footnote"
            };
            let _ = writeln!(
                self.out,
                "<div class=\"bo-note\" id=\"bo-note-{key}\" data-kind=\"{kind}\">"
            );
            self.blocks(blocks, Some(&key));
            self.out.push_str("</div>\n");
            i += 1;
        }
        self.out.push_str("</div>\n");
    }
}

/// A cell edge: the cell's own border, else the table's outer or inside border.
fn pick<'x>(
    own: Option<&'x Border>,
    outer: bool,
    table_outer: Option<&'x Border>,
    inside: Option<&'x Border>,
) -> Option<&'x Border> {
    own.or(if outer { table_outer } else { inside })
}
