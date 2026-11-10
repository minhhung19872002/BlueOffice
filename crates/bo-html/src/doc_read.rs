//! HTML → word-processing document.
//!
//! The subset an office suite is expected to open: headings, paragraphs, inline
//! formatting, lists, tables, pictures and links, formatted by inline CSS and
//! simple `<style>` rules (HTML Living Standard §4 "The elements of HTML"; CSS 2.2
//! property definitions). Scripts are never run or kept (the parser drops them)
//! and nothing is fetched: a picture is imported only when its bytes travel with
//! the page (a `data:` URL, RFC 2397, or a part of the same MHTML file).

use crate::css::{self, Decls, StyleSheet};
use crate::dom::{self, Element, Node};
use crate::image;
use bo_model::{
    Alignment, Block, Border, Borders, Cell, CellProps, CellVAlign, Color, Document, Emu,
    HalfPoints, HeightRule, Image, LineSpacing, NumFormat, NumRef, ParaProps, Paragraph, Row,
    RowProps, Run, RunContent, RunProps, Section, Style, StyleKind, Table, TableProps, TableWidth,
    Twips, Underline, VMerge, VertAlign,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Looks up the bytes of a resource by the URL an element refers to (MHTML parts).
pub type Resolver<'a> = &'a dyn Fn(&str) -> Option<Vec<u8>>;

/// Options of [`read_html_with`].
#[derive(Default, Clone, Copy)]
pub struct ReadOptions<'a> {
    /// Encoding given by the transport (the `charset` of a MIME part).
    pub charset: Option<&'a str>,
    /// Resources that travel with the page.
    pub resolver: Option<Resolver<'a>>,
}

/// Tables nested deeper than this are flattened into paragraphs.
const MAX_TABLE_DEPTH: usize = 6;
/// Most grid columns of a table (the limit of the word-processing model, §17.4).
const MAX_COLS: usize = 63;
/// Most pictures and most picture bytes imported from one page.
const MAX_IMAGES: usize = 4096;
const MAX_IMAGE_BYTES: usize = 256 * 1024 * 1024;
/// Default font size of a page in points (CSS `medium`).
const BASE_PT: f64 = 12.0;

/// Read an HTML file (encoding found by [`bo_encoding::web::decode_html`]).
pub fn read_html(bytes: &[u8]) -> Document {
    read_html_with(bytes, &ReadOptions::default())
}

/// Read an HTML file with a transport charset and travelling resources.
pub fn read_html_with(bytes: &[u8], opts: &ReadOptions<'_>) -> Document {
    read_html_str(
        &bo_encoding::web::decode_html(bytes, opts.charset),
        opts.resolver,
    )
}

/// Read HTML text.
pub fn read_html_str(text: &str, resolver: Option<Resolver<'_>>) -> Document {
    let root = dom::parse(text);
    let mut css = StyleSheet::default();
    let mut notes: HashMap<String, &Element> = HashMap::new();
    for e in root.descendants() {
        if e.name == "style" {
            css.add(&e.text());
        }
        if e.has_class("bo-note") {
            if let Some(id) = e.attr("id") {
                notes.entry(id.to_owned()).or_insert(e);
            }
        }
    }
    let mut doc = Document {
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(24)),
            ..RunProps::default()
        },
        ..Document::default()
    };
    base_styles(&mut doc);
    let page = bo_model::PageSetup::default();
    let mut ctx = Ctx {
        css,
        doc,
        resolver,
        notes,
        used_notes: HashSet::new(),
        images: 0,
        image_bytes: 0,
        content_width: (page.width.0 - page.margin_left.0 - page.margin_right.0).max(1440),
        pending_num: None,
        in_note: false,
        table_depth: 0,
    };
    // The body's own font becomes the document default.
    let body = root.find("body");
    let mut inl = Inline::default();
    if let Some(b) = body {
        let decls = ctx.css.computed(b);
        let mut base = Inline::default();
        apply_inline_css(&decls, &mut base);
        if let Some(f) = base.rp.font.take() {
            ctx.doc.default_run.font = Some(f);
        }
        if let Some(s) = base.rp.size.take() {
            ctx.doc.default_run.size = Some(s);
        }
        if let Some(c) = base.rp.color.take() {
            ctx.doc.default_run.color = Some(c);
        }
        inl.font_pt = base.font_pt;
        inl.pre = base.pre;
    }
    let mut flow = Flow::default();
    let blk = BlockFmt::default();
    ctx.children(body.unwrap_or(&root), &mut flow, &inl, &blk);
    let mut blocks = flow.finish();
    end_with_paragraph(&mut blocks);
    let mut doc = ctx.doc;
    doc.sections = vec![Section::new(blocks)];
    doc
}

/// `Normal`, `Heading1`–`Heading6` and `Hyperlink`.
fn base_styles(doc: &mut Document) {
    let mut normal = Style::new("Normal", "Normal", StyleKind::Paragraph);
    normal.is_default = true;
    doc.styles.styles.push(normal);
    // Sizes of h1–h6 in a browser's default style sheet: 2, 1.5, 1.17, 1, 0.83, 0.67 em.
    for (i, half_points) in [48, 36, 28, 24, 20, 16].into_iter().enumerate() {
        let n = i + 1;
        let mut s = Style::new(
            &format!("Heading{n}"),
            &format!("heading {n}"),
            StyleKind::Paragraph,
        );
        s.based_on = Some("Normal".into());
        s.next = Some("Normal".into());
        s.ppr.keep_next = Some(true);
        s.ppr.outline_level = Some(i as u8);
        s.ppr.space_before = Some(Twips(240));
        s.ppr.space_after = Some(Twips(120));
        s.rpr.bold = Some(true);
        s.rpr.size = Some(HalfPoints(half_points));
        doc.styles.styles.push(s);
    }
    let mut link = Style::new("Hyperlink", "Hyperlink", StyleKind::Character);
    link.rpr.color = Some(Color {
        r: 0x05,
        g: 0x63,
        b: 0xC1,
    });
    link.rpr.underline = Some(Underline::Single);
    doc.styles.styles.push(link);
}

/// A block list must end with a paragraph (§17.4.66 tc, §17.2.2 body).
pub(crate) fn end_with_paragraph(blocks: &mut Vec<Block>) {
    if !matches!(blocks.last(), Some(Block::Paragraph(_))) {
        blocks.push(Block::Paragraph(Paragraph::default()));
    }
}

/// Inherited character formatting.
#[derive(Clone)]
struct Inline {
    rp: RunProps,
    /// White space is kept (`pre`, `white-space: pre-wrap`).
    pre: bool,
    /// Current font size in points (for `em` lengths).
    font_pt: f64,
}

impl Default for Inline {
    fn default() -> Self {
        Inline {
            rp: RunProps::default(),
            pre: false,
            font_pt: BASE_PT,
        }
    }
}

/// Inherited paragraph formatting.
#[derive(Clone, Default)]
struct BlockFmt {
    pp: ParaProps,
    /// Innermost list: numbering instance, level and whether it is a bullet list.
    list: Option<(i32, u8, bool)>,
}

/// A paragraph being filled.
struct ParaBuilder {
    props: ParaProps,
    runs: Vec<Run>,
    /// Nothing visible on the current line yet (leading white space is dropped).
    line_start: bool,
    /// Collapsed white space waiting for the next visible character.
    pending_space: bool,
    /// A `<br>` was seen (an otherwise empty paragraph is kept).
    had_break: bool,
}

/// Output of a block container: finished blocks and the open paragraph.
#[derive(Default)]
struct Flow {
    blocks: Vec<Block>,
    cur: Option<ParaBuilder>,
}

impl Flow {
    fn para(&mut self, props: &ParaProps, num: &mut Option<NumRef>) -> &mut ParaBuilder {
        self.cur.get_or_insert_with(|| {
            let mut props = props.clone();
            if let Some(n) = num.take() {
                props.num = Some(n);
                props.indent_left = None;
                props.indent_first_line = None;
            }
            ParaBuilder {
                props,
                runs: Vec::new(),
                line_start: true,
                pending_space: false,
                had_break: false,
            }
        })
    }

    /// Close the open paragraph. A trailing line break shows nothing (it only ends
    /// the line), and a paragraph without content is dropped unless it had a break.
    fn flush(&mut self) {
        let Some(mut p) = self.cur.take() else { return };
        if let Some(last) = p.runs.last_mut() {
            if last.content.last() == Some(&RunContent::LineBreak) {
                last.content.pop();
                if last.content.is_empty() {
                    p.runs.pop();
                }
            }
        }
        if p.runs.is_empty() && !p.had_break {
            return;
        }
        self.blocks
            .push(Block::Paragraph(Paragraph::new(p.props, p.runs)));
    }

    fn finish(mut self) -> Vec<Block> {
        self.flush();
        self.blocks
    }
}

fn push_content(p: &mut ParaBuilder, rp: &RunProps, item: RunContent) {
    if let Some(last) = p.runs.last_mut() {
        if last.props == *rp {
            if let (RunContent::Text(a), Some(RunContent::Text(b))) =
                (&item, last.content.last_mut())
            {
                b.push_str(a);
            } else {
                last.content.push(item);
            }
            return;
        }
    }
    p.runs.push(Run {
        props: rp.clone(),
        content: vec![item],
    });
}

struct Ctx<'a> {
    css: StyleSheet,
    doc: Document,
    resolver: Option<Resolver<'a>>,
    /// Note bodies written by BlueOffice (`.bo-note`), by element id.
    notes: HashMap<String, &'a Element>,
    used_notes: HashSet<String>,
    images: usize,
    image_bytes: usize,
    /// Width of the text area in twips.
    content_width: i32,
    /// Number of the list item whose first paragraph has not started yet.
    pending_num: Option<NumRef>,
    in_note: bool,
    table_depth: usize,
}

fn pt_to_twips(pt: f64) -> Twips {
    Twips((pt * 20.0).round().clamp(-31_680.0, 31_680.0) as i32)
}

/// Expand a 1–4 value box shorthand to (top, right, bottom, left).
fn box_values(v: &str) -> Option<[&str; 4]> {
    let parts: Vec<&str> = v.split_ascii_whitespace().collect();
    Some(match parts.as_slice() {
        [a] => [a, a, a, a],
        [a, b] => [a, b, a, b],
        [a, b, c] => [a, b, c, b],
        [a, b, c, d] => [a, b, c, d],
        _ => return None,
    })
}

/// One side of `margin` / `padding`: the longhand wins over the shorthand.
fn box_side(decls: &Decls, prop: &str, side: usize, em: f64) -> Option<f64> {
    const SIDES: [&str; 4] = ["top", "right", "bottom", "left"];
    // The later declaration wins.
    let long = format!("{prop}-{}", SIDES[side]);
    for (k, v) in decls.0.iter().rev() {
        if *k == long {
            return css::length_pt(v, em);
        }
        if k == prop {
            return css::length_pt(box_values(v)?[side], em);
        }
    }
    None
}

/// Character formatting declared by CSS.
fn apply_inline_css(d: &Decls, inl: &mut Inline) {
    if let Some(v) = d.get("font-size") {
        if let Some(pt) = css::font_size_pt(v, inl.font_pt) {
            inl.font_pt = pt;
            inl.rp.size = Some(HalfPoints((pt * 2.0).round() as u32));
        }
    }
    if let Some(f) = d.get("font-family").and_then(css::font_family) {
        inl.rp.font = Some(f);
    }
    if let Some(v) = d.get("font-weight") {
        let v = v.to_ascii_lowercase();
        let bold = match v.as_str() {
            "bold" | "bolder" => Some(true),
            "normal" | "lighter" => Some(false),
            n => n.parse::<f64>().ok().map(|w| w >= 600.0),
        };
        if bold.is_some() {
            inl.rp.bold = bold;
        }
    }
    if let Some(v) = d.get("font-style") {
        match v.to_ascii_lowercase().as_str() {
            "italic" | "oblique" => inl.rp.italic = Some(true),
            "normal" => inl.rp.italic = Some(false),
            _ => {}
        }
    }
    if let Some(v) = d
        .get("text-decoration-line")
        .or_else(|| d.get("text-decoration"))
    {
        let v = v.to_ascii_lowercase();
        if v.contains("underline") {
            inl.rp.underline = Some(if v.contains("double") {
                Underline::Double
            } else if v.contains("dotted") {
                Underline::Dotted
            } else {
                Underline::Single
            });
        }
        if v.contains("line-through") {
            inl.rp.strike = Some(true);
        }
        if v.split_ascii_whitespace().next() == Some("none") {
            inl.rp.underline = Some(Underline::None);
            inl.rp.strike = Some(false);
        }
    }
    if let Some(c) = d.get("color").and_then(css::color) {
        inl.rp.color = Some(c);
    }
    if let Some(v) = d.get("vertical-align") {
        match v.to_ascii_lowercase().as_str() {
            "super" => inl.rp.vert_align = Some(VertAlign::Superscript),
            "sub" => inl.rp.vert_align = Some(VertAlign::Subscript),
            "baseline" => inl.rp.vert_align = Some(VertAlign::Baseline),
            _ => {}
        }
    }
    if let Some(v) = d.get("font-variant") {
        match v.to_ascii_lowercase().as_str() {
            "small-caps" => inl.rp.small_caps = Some(true),
            "normal" => inl.rp.small_caps = Some(false),
            _ => {}
        }
    }
    if let Some(v) = d.get("text-transform") {
        match v.to_ascii_lowercase().as_str() {
            "uppercase" => inl.rp.caps = Some(true),
            "none" => inl.rp.caps = Some(false),
            _ => {}
        }
    }
    if let Some(pt) = d
        .get("letter-spacing")
        .and_then(|v| css::length_pt(v, inl.font_pt))
    {
        inl.rp.spacing = Some(pt_to_twips(pt));
    }
    if let Some(v) = d.get("white-space") {
        match v.to_ascii_lowercase().as_str() {
            "pre" | "pre-wrap" | "break-spaces" => inl.pre = true,
            "normal" | "nowrap" | "pre-line" => inl.pre = false,
            _ => {}
        }
    }
}

/// Background colour of an element (`background-color`, or the colour of `background`).
fn background(d: &Decls) -> Option<Color> {
    d.get("background-color").and_then(css::color).or_else(|| {
        d.get("background")
            .and_then(|v| v.split_ascii_whitespace().find_map(css::color))
    })
}

fn alignment(v: &str) -> Option<Alignment> {
    Some(match v.trim().to_ascii_lowercase().as_str() {
        "left" | "start" => Alignment::Left,
        "center" | "middle" => Alignment::Center,
        "right" | "end" => Alignment::Right,
        "justify" => Alignment::Justify,
        _ => return None,
    })
}

/// A border from `border[-side]` (`1px solid #000`), or its width / style / colour
/// longhands. The outer `None`: nothing declared; the inner one: declared as no border.
#[allow(clippy::option_option)]
fn border_side(d: &Decls, side: &str, em: f64) -> Option<Option<Border>> {
    let short =
        d.0.iter()
            .rev()
            .find(|(k, _)| *k == format!("border-{side}") || k == "border")
            .map(|(_, v)| v.as_str());
    let mut width: Option<f64> = None;
    let mut style: Option<String> = None;
    let mut color: Option<Color> = None;
    let mut seen = false;
    for tok in short.into_iter().flat_map(str::split_ascii_whitespace) {
        seen = true;
        let t = tok.to_ascii_lowercase();
        match t.as_str() {
            "none" | "hidden" | "solid" | "double" | "dashed" | "dotted" | "groove" | "ridge"
            | "inset" | "outset" => style = Some(t),
            "thin" => width = Some(0.75),
            "medium" => width = Some(1.5),
            "thick" => width = Some(3.0),
            _ => {
                if let Some(c) = css::color(&t) {
                    color = Some(c);
                } else if let Some(w) = css::length_pt(&t, em) {
                    width = Some(w);
                }
            }
        }
    }
    for (prop, target) in [("style", 0), ("width", 1), ("color", 2)] {
        let v = d
            .get(&format!("border-{side}-{prop}"))
            .or_else(|| d.get(&format!("border-{prop}")));
        if let Some(v) = v {
            seen = true;
            let first = v.split_ascii_whitespace().next().unwrap_or("");
            match target {
                0 => style = Some(first.to_ascii_lowercase()),
                1 => width = css::length_pt(first, em).or(width),
                _ => color = css::color(first).or(color),
            }
        }
    }
    if !seen {
        return None;
    }
    let style = style.unwrap_or_else(|| "none".into());
    if matches!(style.as_str(), "none" | "hidden") || width == Some(0.0) {
        return Some(None);
    }
    let size = (width.unwrap_or(1.5) * 8.0).round().clamp(2.0, 96.0) as u32;
    Some(Some(Border {
        style: match style.as_str() {
            "double" => "double",
            "dashed" => "dashed",
            "dotted" => "dotted",
            _ => "single",
        }
        .into(),
        size,
        color,
        space: 0,
    }))
}

/// The four borders declared by CSS (`None`: no border declaration at all).
fn css_borders(d: &Decls, em: f64) -> Option<Borders> {
    let sides = ["top", "left", "bottom", "right"].map(|s| border_side(d, s, em));
    if sides.iter().all(Option::is_none) {
        return None;
    }
    let [top, left, bottom, right] = sides.map(Option::flatten);
    Some(Borders {
        top,
        left,
        bottom,
        right,
        inside_h: None,
        inside_v: None,
    })
}

/// Paragraph formatting declared by CSS and presentational attributes.
fn apply_block_css(e: &Element, d: &Decls, em: f64, parent: &ParaProps, pp: &mut ParaProps) {
    if let Some(a) = d
        .get("text-align")
        .and_then(alignment)
        .or_else(|| e.attr("align").and_then(alignment))
    {
        pp.align = Some(a);
    }
    let side = |i: usize| -> f64 {
        box_side(d, "margin", i, em).unwrap_or(0.0) + box_side(d, "padding", i, em).unwrap_or(0.0)
    };
    let has = |i: usize| {
        box_side(d, "margin", i, em).is_some() || box_side(d, "padding", i, em).is_some()
    };
    if has(3) {
        let base = parent.indent_left.map_or(0, |t| t.0);
        pp.indent_left = Some(Twips(base + pt_to_twips(side(3)).0));
    }
    if has(1) {
        let base = parent.indent_right.map_or(0, |t| t.0);
        pp.indent_right = Some(Twips(base + pt_to_twips(side(1)).0));
    }
    if let Some(v) = box_side(d, "margin", 0, em) {
        pp.space_before = Some(pt_to_twips(v.max(0.0)));
    }
    if let Some(v) = box_side(d, "margin", 2, em) {
        pp.space_after = Some(pt_to_twips(v.max(0.0)));
    }
    if let Some(v) = d.get("text-indent").and_then(|v| css::length_pt(v, em)) {
        pp.indent_first_line = Some(pt_to_twips(v));
    }
    if let Some(v) = d.get("line-height") {
        let v = v.trim().to_ascii_lowercase();
        if let Ok(k) = v.parse::<f64>() {
            // A multiple of the font size; "single" spacing is about 1.15 of it.
            if k.is_finite() && k > 0.0 {
                let n = (k * 240.0 / LINE_FACTOR).round().clamp(24.0, 31_680.0);
                pp.line_spacing = Some(LineSpacing::Auto(n as u32));
            }
        } else if let Some(pc) = v.strip_suffix('%').and_then(|p| p.parse::<f64>().ok()) {
            if pc.is_finite() && pc > 0.0 {
                let n = (pc / 100.0 * 240.0 / LINE_FACTOR)
                    .round()
                    .clamp(24.0, 31_680.0);
                pp.line_spacing = Some(LineSpacing::Auto(n as u32));
            }
        } else if let Some(pt) = css::length_pt(&v, em) {
            if pt > 0.0 {
                pp.line_spacing = Some(LineSpacing::Exact(pt_to_twips(pt)));
            }
        }
    }
    if let Some(c) = background(d).or_else(|| e.attr("bgcolor").and_then(css::color)) {
        pp.shading = Some(c);
    }
    if let Some(b) = css_borders(d, em) {
        pp.borders = (!b.is_empty()).then_some(b);
    }
    if d.get("page-break-before")
        .or_else(|| d.get("break-before"))
        .is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "always" | "page"))
    {
        pp.page_break_before = Some(true);
    }
}

/// Ratio between a CSS line height of 1 and single line spacing.
pub(crate) const LINE_FACTOR: f64 = 1.15;

/// The element is not shown.
fn is_hidden(e: &Element, d: &Decls) -> bool {
    e.attr("hidden").is_some()
        || d.get("display")
            .is_some_and(|v| v.eq_ignore_ascii_case("none"))
        || d.get("visibility")
            .is_some_and(|v| v.eq_ignore_ascii_case("hidden"))
        || d.get("mso-hide")
            .is_some_and(|v| v.eq_ignore_ascii_case("all"))
}

/// A link target that is safe to keep: web, mail and in-document links only.
fn safe_link(href: &str) -> Option<String> {
    let h = href.trim();
    if h.is_empty() || h.len() > 2048 || h.chars().any(char::is_control) {
        return None;
    }
    let lower = h.to_ascii_lowercase();
    let ok = lower.starts_with('#')
        || ["http://", "https://", "mailto:", "ftp://", "tel:"]
            .iter()
            .any(|p| lower.starts_with(p));
    ok.then(|| h.to_owned())
}

impl<'a> Ctx<'a> {
    fn children(&mut self, e: &'a Element, flow: &mut Flow, inl: &Inline, blk: &BlockFmt) {
        for n in &e.children {
            match n {
                Node::Text(t) => self.text(t, flow, inl, blk),
                Node::Element(c) => self.element(c, flow, inl, blk),
            }
        }
    }

    fn text(&mut self, t: &str, flow: &mut Flow, inl: &Inline, blk: &BlockFmt) {
        if inl.pre {
            // A line break right after the start tag of a preformatted block is not
            // content (HTML §13.2.6.4.7, "pre").
            let t = if flow.cur.is_none() {
                t.strip_prefix("\r\n")
                    .or_else(|| t.strip_prefix('\n'))
                    .unwrap_or(t)
            } else {
                t
            };
            let mut buf = String::new();
            let mut chars = t.chars().peekable();
            while let Some(c) = chars.next() {
                match c {
                    '\r' | '\n' | '\t' => {
                        if c == '\r' && chars.peek() == Some(&'\n') {
                            continue;
                        }
                        let p = flow.para(&blk.pp, &mut self.pending_num);
                        if !buf.is_empty() {
                            push_content(p, &inl.rp, RunContent::Text(std::mem::take(&mut buf)));
                        }
                        let item = if c == '\t' {
                            RunContent::Tab
                        } else {
                            p.had_break = true;
                            RunContent::LineBreak
                        };
                        push_content(p, &inl.rp, item);
                        p.line_start = c != '\t';
                    }
                    _ => buf.push(c),
                }
            }
            if !buf.is_empty() {
                let p = flow.para(&blk.pp, &mut self.pending_num);
                push_content(p, &inl.rp, RunContent::Text(buf));
                p.line_start = false;
            }
            return;
        }
        // Collapse white space (CSS Text §4.1): runs become one space, and a line
        // does not start with one.
        let is_space = |c: char| matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{c}');
        if flow.cur.is_none() && t.chars().all(is_space) {
            return;
        }
        let p = flow.para(&blk.pp, &mut self.pending_num);
        let mut buf = String::new();
        for c in t.chars() {
            if is_space(c) {
                if !p.line_start {
                    p.pending_space = true;
                }
            } else {
                if p.pending_space {
                    buf.push(' ');
                    p.pending_space = false;
                }
                buf.push(c);
                p.line_start = false;
            }
        }
        if !buf.is_empty() {
            push_content(p, &inl.rp, RunContent::Text(buf));
        }
    }

    /// Start a block: close the open paragraph, format the element, lay out its
    /// children as blocks and close again.
    fn block(
        &mut self,
        e: &'a Element,
        flow: &mut Flow,
        inl: &Inline,
        blk: &BlockFmt,
        decls: &Decls,
        setup: impl FnOnce(&mut Inline, &mut BlockFmt),
    ) {
        flow.flush();
        let mut inl2 = Box::new(inl.clone());
        let mut blk2 = Box::new(blk.clone());
        // Vertical spacing, borders and shading belong to one element only.
        blk2.pp.space_before = None;
        blk2.pp.space_after = None;
        blk2.pp.borders = None;
        blk2.pp.style = None;
        blk2.pp.page_break_before = None;
        setup(&mut inl2, &mut blk2);
        apply_inline_css(decls, &mut inl2);
        apply_block_css(e, decls, inl2.font_pt, &blk.pp, &mut blk2.pp);
        self.children(e, flow, &inl2, &blk2);
        flow.flush();
    }

    fn inline(
        &mut self,
        e: &'a Element,
        flow: &mut Flow,
        inl: &Inline,
        blk: &BlockFmt,
        decls: &Decls,
        setup: impl FnOnce(&mut Inline),
    ) {
        let mut inl2 = Box::new(inl.clone());
        setup(&mut inl2);
        apply_inline_css(decls, &mut inl2);
        if let Some(c) = background(decls) {
            // The sixteen highlight colours are highlights; any other colour is shading.
            if Color::from_highlight(c.highlight_name()) == Some(c) {
                inl2.rp.highlight = Some(c);
            } else {
                inl2.rp.shading = Some(c);
            }
        }
        self.children(e, flow, &inl2, blk);
    }

    #[allow(clippy::too_many_lines)]
    fn element(&mut self, e: &'a Element, flow: &mut Flow, inl: &Inline, blk: &BlockFmt) {
        let decls = self.css.computed(e);
        if is_hidden(e, &decls) || e.has_class("bo-notes") || e.has_class("bo-notes-sep") {
            return;
        }
        let name = e.name.as_str();
        match name {
            "head" | "style" | "title" | "meta" | "link" | "base" | "colgroup" | "col"
            | "caption" | "datalist" | "option" | "optgroup" | "button" | "input" | "textarea"
            | "map" | "area" | "param" | "source" | "track" | "embed" | "frame" | "frameset"
            | "dialog" | "menu" | "rp" | "wbr" => {}
            "br" => {
                let page = decls
                    .get("page-break-before")
                    .or_else(|| decls.get("break-before"))
                    .is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "always" | "page"));
                let p = flow.para(&blk.pp, &mut self.pending_num);
                p.pending_space = false;
                if page {
                    push_content(p, &inl.rp, RunContent::PageBreak);
                } else {
                    push_content(p, &inl.rp, RunContent::LineBreak);
                    p.had_break = true;
                }
                p.line_start = true;
            }
            "hr" => {
                flow.flush();
                let mut pp = blk.pp.clone();
                pp.num = None;
                pp.borders = Some(Borders {
                    bottom: Some(Border::single(6)),
                    ..Borders::default()
                });
                flow.blocks
                    .push(Block::Paragraph(Paragraph::new(pp, Vec::new())));
            }
            "img" => self.img(e, flow, inl, blk, &decls),
            "p" => self.block(e, flow, inl, blk, &decls, |_, b| {
                b.pp.space_after = Some(Twips(200));
            }),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                let n = usize::from(name.as_bytes()[1] - b'0');
                let sizes = [24.0, 18.0, 14.0, 12.0, 10.0, 8.0];
                self.block(e, flow, inl, blk, &decls, |i, b| {
                    b.pp.style = Some(format!("Heading{n}"));
                    // The style carries size and weight; `em` lengths use its size.
                    i.font_pt = sizes[n - 1];
                    i.rp.size = None;
                    i.rp.bold = None;
                });
            }
            "div" | "section" | "article" | "header" | "footer" | "main" | "nav" | "aside"
            | "figure" | "figcaption" | "address" | "form" | "fieldset" | "details" | "summary"
            | "dl" | "dt" | "body" | "html" | "legend" => {
                self.block(e, flow, inl, blk, &decls, |_, _| {});
            }
            "center" => self.block(e, flow, inl, blk, &decls, |_, b| {
                b.pp.align = Some(Alignment::Center);
            }),
            "blockquote" | "dd" => {
                let quote = name == "blockquote";
                self.block(e, flow, inl, blk, &decls, |_, b| {
                    b.pp.indent_left = Some(Twips(b.pp.indent_left.map_or(0, |t| t.0) + 720));
                    if quote {
                        b.pp.indent_right = Some(Twips(b.pp.indent_right.map_or(0, |t| t.0) + 720));
                    }
                });
            }
            "pre" | "listing" | "xmp" | "plaintext" => {
                self.block(e, flow, inl, blk, &decls, |i, _| {
                    i.pre = true;
                    i.rp.font = Some("Courier New".into());
                    i.rp.size = Some(HalfPoints(20));
                    i.font_pt = 10.0;
                });
            }
            "ul" | "ol" | "dir" => self.list(e, flow, inl, blk, &decls),
            "li" => {
                if let Some((num_id, ilvl, _)) = blk.list {
                    flow.flush();
                    self.pending_num = Some(NumRef { num_id, ilvl });
                    let indent = Twips(720 * (i32::from(ilvl) + 1));
                    self.block(e, flow, inl, blk, &decls, |_, b| {
                        // Later paragraphs of the item line up with its text.
                        b.pp.indent_left = Some(indent);
                        b.pp.indent_first_line = None;
                    });
                    self.pending_num = None;
                } else {
                    self.block(e, flow, inl, blk, &decls, |_, _| {});
                }
            }
            "table" => self.table(e, flow, inl, blk, &decls),
            // Table parts outside a table: their content only.
            "thead" | "tbody" | "tfoot" | "tr" | "td" | "th" => {
                self.block(e, flow, inl, blk, &decls, |_, _| {});
            }
            "a" => self.anchor(e, flow, inl, blk, &decls),
            "b" | "strong" => self.inline(e, flow, inl, blk, &decls, |i| i.rp.bold = Some(true)),
            "i" | "em" | "cite" | "var" | "dfn" => {
                self.inline(e, flow, inl, blk, &decls, |i| i.rp.italic = Some(true));
            }
            "u" | "ins" => self.inline(e, flow, inl, blk, &decls, |i| {
                i.rp.underline = Some(Underline::Single);
            }),
            "s" | "strike" | "del" => {
                self.inline(e, flow, inl, blk, &decls, |i| i.rp.strike = Some(true));
            }
            "sup" => self.inline(e, flow, inl, blk, &decls, |i| {
                i.rp.vert_align = Some(VertAlign::Superscript);
            }),
            "sub" => self.inline(e, flow, inl, blk, &decls, |i| {
                i.rp.vert_align = Some(VertAlign::Subscript);
            }),
            "code" | "tt" | "kbd" | "samp" => self.inline(e, flow, inl, blk, &decls, |i| {
                i.rp.font = Some("Courier New".into());
            }),
            "mark" => self.inline(e, flow, inl, blk, &decls, |i| {
                i.rp.highlight = Color::from_highlight("yellow");
            }),
            "small" | "big" => {
                let k = if name == "small" { 1.0 / 1.2 } else { 1.2 };
                self.inline(e, flow, inl, blk, &decls, |i| {
                    i.font_pt = (i.font_pt * k).clamp(1.0, 1638.0);
                    i.rp.size = Some(HalfPoints((i.font_pt * 2.0).round() as u32));
                });
            }
            "font" => {
                let color = e.attr("color").and_then(css::color);
                let face = e.attr("face").and_then(css::font_family);
                let size = e.attr("size").and_then(css::legacy_font_size_pt);
                self.inline(e, flow, inl, blk, &decls, |i| {
                    if color.is_some() {
                        i.rp.color = color;
                    }
                    if face.is_some() {
                        i.rp.font = face;
                    }
                    if let Some(pt) = size {
                        i.font_pt = pt;
                        i.rp.size = Some(HalfPoints((pt * 2.0).round() as u32));
                    }
                });
            }
            // Everything else is a transparent inline container.
            _ => self.inline(e, flow, inl, blk, &decls, |_| {}),
        }
    }

    fn anchor(
        &mut self,
        e: &'a Element,
        flow: &mut Flow,
        inl: &Inline,
        blk: &BlockFmt,
        decls: &Decls,
    ) {
        let href = e.attr("href").unwrap_or("");
        // A note reference written by BlueOffice: the note comes back as a note.
        if e.has_class("bo-noteref") && !self.in_note {
            if let Some(id) = href.strip_prefix('#') {
                if let Some(&note) = self.notes.get(id) {
                    if self.used_notes.insert(id.to_owned()) {
                        self.note(note, flow, inl, blk);
                        return;
                    }
                }
            }
        }
        if e.has_class("bo-notemark") && self.in_note {
            let mut rp = inl.rp.clone();
            rp.vert_align = Some(VertAlign::Superscript);
            let p = flow.para(&blk.pp, &mut self.pending_num);
            push_content(p, &rp, RunContent::NoteMark);
            p.line_start = false;
            return;
        }
        let link = safe_link(href);
        self.inline(e, flow, inl, blk, decls, |i| {
            if link.is_some() {
                i.rp.link = link;
                i.rp.style = Some("Hyperlink".into());
            }
        });
    }

    /// Bring a `.bo-note` body back as a footnote or endnote and reference it.
    fn note(&mut self, note: &'a Element, flow: &mut Flow, inl: &Inline, blk: &BlockFmt) {
        let endnote = note.attr("data-kind") == Some("endnote");
        let mut body = Flow::default();
        let saved_num = self.pending_num.take();
        self.in_note = true;
        self.children(note, &mut body, &Inline::default(), &BlockFmt::default());
        self.in_note = false;
        self.pending_num = saved_num;
        let mut blocks = body.finish();
        end_with_paragraph(&mut blocks);
        let map = if endnote {
            &mut self.doc.endnotes
        } else {
            &mut self.doc.footnotes
        };
        let id = map
            .keys()
            .next_back()
            .map_or(1, |k| k.saturating_add(1))
            .max(1);
        map.insert(id, blocks);
        let mut rp = inl.rp.clone();
        rp.vert_align = Some(VertAlign::Superscript);
        let p = flow.para(&blk.pp, &mut self.pending_num);
        if p.pending_space {
            push_content(p, &inl.rp, RunContent::Text(" ".into()));
            p.pending_space = false;
        }
        push_content(
            p,
            &rp,
            if endnote {
                RunContent::EndnoteRef(id)
            } else {
                RunContent::FootnoteRef(id)
            },
        );
        p.line_start = false;
    }

    fn list(
        &mut self,
        e: &'a Element,
        flow: &mut Flow,
        inl: &Inline,
        blk: &BlockFmt,
        decls: &Decls,
    ) {
        flow.flush();
        let css_type = decls
            .get("list-style-type")
            .or_else(|| decls.get("list-style"))
            .map(str::to_ascii_lowercase);
        let ordered = e.name == "ol";
        let format = match (css_type.as_deref(), e.attr("type")) {
            (Some("decimal"), _) | (None, Some("1")) => Some(NumFormat::Decimal),
            (Some("lower-alpha" | "lower-latin"), _) | (None, Some("a")) => {
                Some(NumFormat::LowerLetter)
            }
            (Some("upper-alpha" | "upper-latin"), _) | (None, Some("A")) => {
                Some(NumFormat::UpperLetter)
            }
            (Some("lower-roman"), _) | (None, Some("i")) => Some(NumFormat::LowerRoman),
            (Some("upper-roman"), _) | (None, Some("I")) => Some(NumFormat::UpperRoman),
            _ => None,
        };
        let bullet = !ordered && format.is_none();
        let start = e
            .attr("start")
            .and_then(|s| s.trim().parse::<i32>().ok())
            .filter(|s| (0..=32_767).contains(s));
        // A nested list of the same kind continues the numbering instance one level
        // down; anything else starts an instance of its own.
        let (num_id, ilvl) = match blk.list {
            Some((id, lvl, b)) if b == bullet && format.is_none() && start.is_none() => {
                (id, (lvl + 1).min(8))
            }
            other => {
                let lvl = other.map_or(0, |(_, l, _)| (l + 1).min(8));
                // One numbering instance per list, capped: a page of thousands of
                // lists shares the last ones.
                let id = if self.doc.numbering.nums.len() < 2000 {
                    self.doc.numbering.add_simple_list(bullet)
                } else {
                    self.doc.numbering.nums.last().map_or(1, |n| n.id)
                };
                (id, lvl)
            }
        };
        if format.is_some() || start.is_some() {
            let abs = self.doc.numbering.abstract_of(num_id);
            if let Some(level) = self
                .doc
                .numbering
                .abstracts
                .iter_mut()
                .find(|a| Some(a.id) == abs)
                .and_then(|a| a.levels.iter_mut().find(|l| l.ilvl == ilvl))
            {
                if let Some(f) = format {
                    level.format = f;
                    level.text = format!("%{}.", ilvl + 1);
                }
                if let Some(s) = start {
                    level.start = s;
                }
            }
        }
        let mut blk2 = Box::new(blk.clone());
        blk2.list = Some((num_id, ilvl, bullet));
        blk2.pp.space_before = None;
        blk2.pp.space_after = None;
        let mut inl2 = Box::new(inl.clone());
        apply_inline_css(decls, &mut inl2);
        let saved = self.pending_num.take();
        self.children(e, flow, &inl2, &blk2);
        flow.flush();
        self.pending_num = saved;
    }

    fn img(&mut self, e: &Element, flow: &mut Flow, inl: &Inline, blk: &BlockFmt, decls: &Decls) {
        let alt = e.attr("alt").unwrap_or("").trim().to_owned();
        let bytes = e
            .attr("src")
            .and_then(|src| image::data_url(src).or_else(|| self.resolver.and_then(|r| r(src))));
        let picture = bytes.and_then(|b| image::sniff(&b).map(|info| (b, info)));
        let Some((bytes, info)) = picture else {
            // Not travelling with the page (or not a picture): its description stays.
            if !alt.is_empty() {
                self.text(&alt, flow, inl, blk);
            }
            return;
        };
        if self.images >= MAX_IMAGES || self.image_bytes + bytes.len() > MAX_IMAGE_BYTES {
            return;
        }
        // Size: CSS, then attributes (pixels), then the picture's own size.
        let dim = |css_prop: &str, attr: &str| -> Option<f64> {
            decls
                .get(css_prop)
                .filter(|v| !v.contains('%'))
                .and_then(|v| css::length_pt(v, inl.font_pt))
                .or_else(|| {
                    e.attr(attr)
                        .filter(|v| !v.contains('%'))
                        .and_then(|v| css::length_pt(v, inl.font_pt))
                })
                .filter(|v| *v > 0.0)
        };
        let (iw, ih) = (
            f64::from(info.width.max(1)) * 0.75,
            f64::from(info.height.max(1)) * 0.75,
        );
        let (mut w, mut h) = match (dim("width", "width"), dim("height", "height")) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => (w, w * ih / iw),
            (None, Some(h)) => (h * iw / ih, h),
            (None, None) => (iw, ih),
        };
        let max_w = f64::from(self.content_width) / 20.0;
        if w > max_w {
            h *= max_w / w;
            w = max_w;
        }
        let h = h.clamp(0.75, 1584.0);
        // The same bytes are stored once.
        let existing = self
            .doc
            .media
            .iter()
            .find(|(_, v)| v.as_slice() == bytes.as_slice())
            .map(|(k, _)| k.clone());
        let media = existing.unwrap_or_else(|| {
            self.images += 1;
            self.image_bytes += bytes.len();
            let name = format!("/word/media/image{}.{}", self.images, info.ext);
            self.doc.media.insert(name.clone(), Arc::new(bytes));
            name
        });
        let p = flow.para(&blk.pp, &mut self.pending_num);
        if p.pending_space {
            push_content(p, &inl.rp, RunContent::Text(" ".into()));
            p.pending_space = false;
        }
        push_content(
            p,
            &inl.rp,
            RunContent::Image(Box::new(Image {
                media,
                width: Emu((w * 12_700.0).round() as i64),
                height: Emu((h * 12_700.0).round() as i64),
                anchor: None,
                crop: None,
                rotation: 0,
                description: alt,
                shape: None,
                format: None,
                ole: None,
                src: None,
            })),
        );
        p.line_start = false;
    }

    #[allow(clippy::too_many_lines)]
    fn table(
        &mut self,
        e: &'a Element,
        flow: &mut Flow,
        inl: &Inline,
        blk: &BlockFmt,
        decls: &Decls,
    ) {
        flow.flush();
        if self.table_depth >= MAX_TABLE_DEPTH {
            // Too deep: the cells' content follows as paragraphs.
            self.children(e, flow, inl, blk);
            flow.flush();
            return;
        }
        let mut inl_t = Box::new(inl.clone());
        apply_inline_css(decls, &mut inl_t);
        let em = inl_t.font_pt;
        // Caption: a centred paragraph above the table.
        if let Some(cap) = e.elements().find(|c| c.name == "caption") {
            let mut b = Box::new(blk.clone());
            b.pp.align = Some(Alignment::Center);
            b.pp.num = None;
            self.children(cap, flow, &inl_t, &b);
            flow.flush();
        }
        // Rows in document order, `thead` rows marked as header rows.
        let mut rows: Vec<(&'a Element, bool)> = Vec::new();
        let mut col_widths: Vec<Option<f64>> = Vec::new();
        for c in e.elements() {
            match c.name.as_str() {
                "tr" => rows.push((c, false)),
                "thead" | "tbody" | "tfoot" => {
                    let head = c.name == "thead";
                    rows.extend(c.elements().filter(|r| r.name == "tr").map(|r| (r, head)));
                }
                "colgroup" | "col" => {
                    let cols: Vec<&Element> = if c.name == "col" {
                        vec![c]
                    } else {
                        c.elements().filter(|x| x.name == "col").collect()
                    };
                    for col in cols {
                        let span = col
                            .attr("span")
                            .and_then(|s| s.parse::<usize>().ok())
                            .unwrap_or(1)
                            .clamp(1, MAX_COLS);
                        let w = self.width_pt(col, em);
                        for _ in 0..span {
                            if col_widths.len() < MAX_COLS {
                                col_widths.push(w);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if rows.is_empty() {
            return;
        }
        let table_border = e
            .attr("border")
            .is_some_and(|b| b.trim().parse::<f64>().map_or(!b.is_empty(), |n| n > 0.0));
        let n_rows = rows.len();
        // Columns still covered by a row-spanning cell: remaining rows and span width.
        let mut spans: Vec<(usize, u32)> = vec![(0, 0); MAX_COLS];
        let mut out_rows: Vec<Row> = Vec::with_capacity(n_rows.min(4096));
        let mut n_cols = 0usize;
        self.table_depth += 1;
        for (ri, (tr, head)) in rows.iter().enumerate() {
            let tr_decls = self.css.computed(tr);
            let mut inl_r = Box::new((*inl_t).clone());
            apply_inline_css(&tr_decls, &mut inl_r);
            let row_bg = background(&tr_decls).or_else(|| tr.attr("bgcolor").and_then(css::color));
            let row_align = tr_decls
                .get("text-align")
                .and_then(alignment)
                .or_else(|| tr.attr("align").and_then(alignment));
            let mut cells: Vec<Cell> = Vec::new();
            let mut col = 0usize;
            let fill_spans =
                |col: &mut usize, cells: &mut Vec<Cell>, spans: &mut Vec<(usize, u32)>| {
                    while *col < MAX_COLS && spans[*col].0 > 0 {
                        let width = spans[*col].1.max(1);
                        spans[*col].0 -= 1;
                        cells.push(Cell {
                            props: CellProps {
                                grid_span: width,
                                v_merge: Some(VMerge::Continue),
                                ..CellProps::default()
                            },
                            blocks: vec![Block::Paragraph(Paragraph::default())],
                        });
                        *col += width as usize;
                    }
                };
            for td in tr
                .elements()
                .filter(|c| matches!(c.name.as_str(), "td" | "th"))
            {
                fill_spans(&mut col, &mut cells, &mut spans);
                if col >= MAX_COLS {
                    break;
                }
                let td_decls = self.css.computed(td);
                if is_hidden(td, &td_decls) {
                    continue;
                }
                let int_attr =
                    |name: &str| td.attr(name).and_then(|v| v.trim().parse::<usize>().ok());
                let colspan = int_attr("colspan").unwrap_or(1).clamp(1, MAX_COLS - col);
                let rowspan = match int_attr("rowspan") {
                    Some(0) => n_rows - ri,
                    Some(n) => n.min(n_rows - ri),
                    None => 1,
                };
                let mut props = CellProps {
                    grid_span: colspan as u32,
                    ..CellProps::default()
                };
                if rowspan > 1 {
                    props.v_merge = Some(VMerge::Restart);
                    spans[col] = (rowspan - 1, colspan as u32);
                }
                props.shading = background(&td_decls)
                    .or_else(|| td.attr("bgcolor").and_then(css::color))
                    .or(row_bg);
                props.v_align = td_decls
                    .get("vertical-align")
                    .or_else(|| td.attr("valign"))
                    .and_then(|v| match v.to_ascii_lowercase().as_str() {
                        "top" => Some(CellVAlign::Top),
                        "middle" | "center" => Some(CellVAlign::Center),
                        "bottom" => Some(CellVAlign::Bottom),
                        _ => None,
                    });
                props.borders = css_borders(&td_decls, em).filter(|b| !b.is_empty());
                if ri == 0 || col_widths.get(col).is_none_or(Option::is_none) {
                    // A width on a cell sizes its column(s).
                    if let Some(w) = self.width_pt(td, em) {
                        let each = w / colspan as f64;
                        for c in col..col + colspan {
                            if col_widths.len() <= c {
                                col_widths.resize(c + 1, None);
                            }
                            if col_widths[c].is_none() {
                                col_widths[c] = Some(each);
                            }
                        }
                    }
                }
                let mut inl_c = Box::new((*inl_r).clone());
                let mut blk_c = Box::new(BlockFmt::default());
                blk_c.pp.align = row_align;
                if td.name == "th" {
                    inl_c.rp.bold = Some(true);
                    blk_c.pp.align = Some(Alignment::Center);
                }
                apply_inline_css(&td_decls, &mut inl_c);
                if let Some(a) = td_decls
                    .get("text-align")
                    .and_then(alignment)
                    .or_else(|| td.attr("align").and_then(alignment))
                {
                    blk_c.pp.align = Some(a);
                }
                let mut cell_flow = Flow::default();
                let saved = self.pending_num.take();
                self.children(td, &mut cell_flow, &inl_c, &blk_c);
                self.pending_num = saved;
                let mut blocks = cell_flow.finish();
                end_with_paragraph(&mut blocks);
                cells.push(Cell { props, blocks });
                col += colspan;
            }
            fill_spans(&mut col, &mut cells, &mut spans);
            n_cols = n_cols.max(col);
            out_rows.push(Row {
                props: RowProps {
                    header: *head,
                    height: self
                        .css
                        .computed(tr)
                        .get("height")
                        .and_then(|v| css::length_pt(v, em))
                        .filter(|h| *h > 0.0)
                        .map(|h| (pt_to_twips(h), HeightRule::AtLeast)),
                    ..RowProps::default()
                },
                cells,
            });
        }
        self.table_depth -= 1;
        let n_cols = n_cols.clamp(1, MAX_COLS);
        // Every row covers the whole grid.
        for row in &mut out_rows {
            let mut covered: usize = row.cells.iter().map(|c| c.props.grid_span as usize).sum();
            while covered < n_cols {
                row.cells.push(Cell {
                    props: CellProps::default(),
                    blocks: vec![Block::Paragraph(Paragraph::default())],
                });
                covered += 1;
            }
        }
        // Column widths: declared ones, the rest share what is left of the table width.
        let avail = f64::from(self.content_width) / 20.0;
        let table_w = e
            .attr("width")
            .or_else(|| decls.get("width"))
            .and_then(|v| match v.trim().strip_suffix('%') {
                Some(pc) => pc.parse::<f64>().ok().map(|p| avail * p / 100.0),
                None => css::length_pt(v, em),
            })
            .filter(|w| *w > 0.0)
            .map_or(avail, |w| w.min(avail));
        col_widths.resize(n_cols, None);
        let declared: f64 = col_widths.iter().flatten().map(|w| w.max(1.0)).sum();
        let free = col_widths.iter().filter(|w| w.is_none()).count();
        let share = if free > 0 {
            ((table_w - declared) / free as f64).max(table_w / (n_cols as f64 * 4.0))
        } else {
            0.0
        };
        let mut widths: Vec<f64> = col_widths
            .iter()
            .map(|w| w.map_or(share, |w| w.max(1.0)))
            .collect();
        let total: f64 = widths.iter().sum();
        if total > avail && total > 0.0 {
            let k = avail / total;
            for w in &mut widths {
                *w *= k;
            }
        }
        let grid: Vec<Twips> = widths.iter().map(|w| pt_to_twips(*w)).collect();
        for row in &mut out_rows {
            let mut c = 0usize;
            for cell in &mut row.cells {
                let span = (cell.props.grid_span as usize).max(1);
                let w: i32 = grid.iter().skip(c).take(span).map(|t| t.0).sum();
                cell.props.width = Some(TableWidth::Dxa(Twips(w)));
                c += span;
            }
        }
        let css_table_borders = css_borders(decls, em).filter(|b| !b.is_empty());
        let borders = if table_border {
            Some(Borders::all(&Border::single(4)))
        } else {
            css_table_borders
        };
        let align = e
            .attr("align")
            .and_then(alignment)
            .or_else(|| {
                let auto = |p: &str| decls.get(p).is_some_and(|v| v.eq_ignore_ascii_case("auto"));
                let both = decls.get("margin").and_then(box_values).is_some_and(|m| {
                    m[1].eq_ignore_ascii_case("auto") && m[3].eq_ignore_ascii_case("auto")
                });
                (both || (auto("margin-left") && auto("margin-right"))).then_some(Alignment::Center)
            })
            .filter(|a| *a != Alignment::Justify);
        flow.blocks.push(Block::Table(Table {
            props: TableProps {
                width: Some(TableWidth::Dxa(Twips(grid.iter().map(|t| t.0).sum()))),
                align,
                borders,
                ..TableProps::default()
            },
            grid,
            grid_change: None,
            rows: out_rows,
            src: None,
        }));
    }

    /// Declared width of a column or cell in points (percentages of the text area).
    fn width_pt(&self, e: &Element, em: f64) -> Option<f64> {
        let decls = self.css.computed(e);
        let v = decls.get("width").or_else(|| e.attr("width"))?;
        let avail = f64::from(self.content_width) / 20.0;
        match v.trim().strip_suffix('%') {
            Some(pc) => pc.parse::<f64>().ok().map(|p| avail * p / 100.0),
            None => css::length_pt(v, em),
        }
        .filter(|w| w.is_finite() && *w > 0.0)
    }
}
