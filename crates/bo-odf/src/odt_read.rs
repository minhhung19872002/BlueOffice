//! `.odt` reader → [`bo_model::Document`].

use crate::dom::{self, Element, Node};
use crate::ns::{FO, OFFICE, STYLE, SVG, TABLE, TEXT};
use crate::package::OdfPackage;
use crate::units::{parse_length, parse_percent};
use crate::{OdfError, Result, MIME_ODT};
use bo_model::{
    Alignment, Block, Color, Document, HalfPoints, LineSpacing, PageSetup, ParaProps, Paragraph,
    Run, RunContent, RunProps, Section, Underline,
};
use std::collections::HashMap;

/// Maximum `style:parent-style-name` chain followed.
const MAX_STYLE_DEPTH: usize = 32;

/// A style as declared (ODF 1.3 Part 3, §16.2 `style:style`).
#[derive(Debug, Clone, Default)]
struct StyleDef {
    parent: Option<String>,
    para: ParaProps,
    text: RunProps,
    master_page: Option<String>,
    /// fo:break-after="page" (§20.191).
    break_after: bool,
}

#[derive(Debug, Default)]
struct Styles {
    /// (family, name) → definition.
    defs: HashMap<(String, String), StyleDef>,
    /// font-face name → family (§16.23 `style:font-face`).
    fonts: HashMap<String, String>,
    page_layouts: HashMap<String, PageSetup>,
    /// master page name → page layout name.
    masters: Vec<(String, String)>,
    default_para: ParaProps,
    default_text: RunProps,
}

impl Styles {
    fn collect_fonts(&mut self, root: &Element) {
        if let Some(decls) = root.child(OFFICE, "font-face-decls") {
            for f in decls.children_named(STYLE, "font-face") {
                if let (Some(name), Some(family)) =
                    (f.attr(STYLE, "name"), f.attr(SVG, "font-family"))
                {
                    let family = family.trim_matches(|c| c == '\'' || c == '"').to_owned();
                    self.fonts.insert(name.to_owned(), family);
                }
            }
        }
    }

    fn collect(&mut self, container: &Element) {
        for e in container.elements() {
            if e.is(STYLE, "style") {
                let (Some(name), Some(family)) = (e.attr(STYLE, "name"), e.attr(STYLE, "family"))
                else {
                    continue;
                };
                let def = StyleDef {
                    parent: e.attr(STYLE, "parent-style-name").map(str::to_owned),
                    para: e
                        .child(STYLE, "paragraph-properties")
                        .map(para_props)
                        .unwrap_or_default(),
                    text: e
                        .child(STYLE, "text-properties")
                        .map(|t| text_props(t, &self.fonts))
                        .unwrap_or_default(),
                    master_page: e
                        .attr(STYLE, "master-page-name")
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned),
                    break_after: e
                        .child(STYLE, "paragraph-properties")
                        .and_then(|p| p.attr(FO, "break-after"))
                        == Some("page"),
                };
                self.defs.insert((family.to_owned(), name.to_owned()), def);
            } else if e.is(STYLE, "default-style") && e.attr(STYLE, "family") == Some("paragraph") {
                if let Some(p) = e.child(STYLE, "paragraph-properties") {
                    self.default_para = para_props(p).inherit(&self.default_para);
                }
                if let Some(t) = e.child(STYLE, "text-properties") {
                    self.default_text = text_props(t, &self.fonts).inherit(&self.default_text);
                }
            } else if e.is(STYLE, "page-layout") {
                if let (Some(name), Some(props)) = (
                    e.attr(STYLE, "name"),
                    e.child(STYLE, "page-layout-properties"),
                ) {
                    self.page_layouts.insert(name.to_owned(), page_props(props));
                }
            }
        }
    }

    fn collect_masters(&mut self, root: &Element) {
        if let Some(ms) = root.child(OFFICE, "master-styles") {
            for m in ms.children_named(STYLE, "master-page") {
                if let (Some(name), Some(layout)) =
                    (m.attr(STYLE, "name"), m.attr(STYLE, "page-layout-name"))
                {
                    self.masters.push((name.to_owned(), layout.to_owned()));
                }
            }
        }
    }

    /// Resolved (paragraph props, text props, master page, break-after) of a style.
    fn resolve(&self, family: &str, name: Option<&str>) -> StyleDef {
        let mut chain = Vec::new();
        let mut cur = name.map(str::to_owned);
        while let Some(n) = cur {
            if chain.len() >= MAX_STYLE_DEPTH {
                break;
            }
            let Some(def) = self.defs.get(&(family.to_owned(), n)) else {
                break;
            };
            chain.push(def);
            cur = def.parent.clone();
        }
        let mut out = StyleDef::default();
        // Overlay from the most-derived style down to the root.
        for (i, def) in chain.iter().enumerate() {
            out.para = out.para.inherit(&def.para);
            out.text = out.text.inherit(&def.text);
            if i == 0 {
                out.master_page.clone_from(&def.master_page);
                out.break_after = def.break_after;
            }
        }
        out
    }

    fn page(&self, master: &str) -> Option<PageSetup> {
        let layout = self
            .masters
            .iter()
            .find(|(n, _)| n == master)
            .map(|(_, l)| l)?;
        self.page_layouts.get(layout).copied()
    }

    fn default_page(&self) -> PageSetup {
        self.page("Standard")
            .or_else(|| self.masters.first().and_then(|(n, _)| self.page(n)))
            .unwrap_or_default()
    }
}

/// `style:paragraph-properties` (ODF 1.3 Part 3, §17.6).
fn para_props(e: &Element) -> ParaProps {
    let len = |ns: &str, n: &str| e.attr(ns, n).and_then(parse_length);
    // §20.216 fo:text-align
    let align = e.attr(FO, "text-align").and_then(|a| match a {
        "start" | "left" => Some(Alignment::Left),
        "end" | "right" => Some(Alignment::Right),
        "center" => Some(Alignment::Center),
        "justify" => Some(Alignment::Justify),
        _ => None,
    });
    // §20.204 fo:line-height, §20.317 style:line-height-at-least
    let line_spacing = if let Some(lh) = e.attr(FO, "line-height") {
        if lh == "normal" {
            Some(LineSpacing::Auto(240))
        } else if let Some(p) = parse_percent(lh) {
            Some(LineSpacing::Auto((p * 2.4).round().clamp(0.0, 1e6) as u32))
        } else {
            parse_length(lh).map(LineSpacing::Exact)
        }
    } else {
        len(STYLE, "line-height-at-least").map(LineSpacing::AtLeast)
    };
    ParaProps {
        align,
        indent_left: len(FO, "margin-left"),
        indent_right: len(FO, "margin-right"),
        indent_first_line: len(FO, "text-indent"),
        space_before: len(FO, "margin-top"),
        space_after: len(FO, "margin-bottom"),
        line_spacing,
        // §20.186 fo:break-before
        page_break_before: e.attr(FO, "break-before").map(|b| b == "page"),
    }
}

/// `style:text-properties` (ODF 1.3 Part 3, §17.10).
fn text_props(e: &Element, fonts: &HashMap<String, String>) -> RunProps {
    // §20.190 fo:font-weight
    let bold = e
        .attr(FO, "font-weight")
        .map(|w| w == "bold" || w.parse::<u32>().is_ok_and(|n| n >= 600));
    // §20.188 fo:font-style
    let italic = e
        .attr(FO, "font-style")
        .map(|s| s == "italic" || s == "oblique");
    // §20.187 fo:font-size (absolute lengths only)
    let size = e
        .attr(FO, "font-size")
        .and_then(parse_length)
        .filter(|t| t.0 > 0)
        .map(|t| HalfPoints(((t.0 + 5) / 10) as u32));
    // §20.282 style:font-name, §20.185 fo:font-family
    let font = e
        .attr(STYLE, "font-name")
        .map(|n| fonts.get(n).cloned().unwrap_or_else(|| n.to_owned()))
        .or_else(|| {
            e.attr(FO, "font-family")
                .map(|f| f.trim_matches(|c| c == '\'' || c == '"').to_owned())
        });
    // §20.389 style:text-underline-style, §20.390 style:text-underline-type
    let underline = e.attr(STYLE, "text-underline-style").map(|s| {
        if s == "none" {
            Underline::None
        } else if e.attr(STYLE, "text-underline-type") == Some("double") {
            Underline::Double
        } else {
            Underline::Single
        }
    });
    // §20.375 style:text-line-through-style
    let strike = e
        .attr(STYLE, "text-line-through-style")
        .map(|s| s != "none");
    // §20.180 fo:color
    let color = e
        .attr(FO, "color")
        .and_then(|c| c.strip_prefix('#'))
        .and_then(Color::from_hex);
    RunProps {
        font,
        size,
        bold,
        italic,
        underline,
        strike,
        color,
    }
}

/// `style:page-layout-properties` (ODF 1.3 Part 3, §17.2).
fn page_props(e: &Element) -> PageSetup {
    let d = PageSetup::default();
    let len = |n: &str, def| e.attr(FO, n).and_then(parse_length).unwrap_or(def);
    PageSetup {
        width: len("page-width", d.width),
        height: len("page-height", d.height),
        margin_top: len("margin-top", d.margin_top),
        margin_bottom: len("margin-bottom", d.margin_bottom),
        margin_left: len("margin-left", d.margin_left),
        margin_right: len("margin-right", d.margin_right),
    }
}

/// Builds runs for one paragraph, merging adjacent runs with equal props.
struct RunBuilder {
    runs: Vec<Run>,
    /// Whether the previous emitted character was white space (§6.1.2 collapsing).
    last_space: bool,
}

impl RunBuilder {
    fn push(&mut self, props: &RunProps, item: RunContent) {
        if let (Some(last), RunContent::Text(t)) = (self.runs.last_mut(), &item) {
            if last.props == *props {
                if let Some(RunContent::Text(prev)) = last.content.last_mut() {
                    prev.push_str(t);
                    return;
                }
            }
        }
        match self.runs.last_mut() {
            Some(last) if last.props == *props => last.content.push(item),
            _ => self.runs.push(Run {
                props: props.clone(),
                content: vec![item],
            }),
        }
    }

    /// Text node: white-space sequences collapse to one space (ODF 1.3 Part 3, §6.1.2).
    fn text(&mut self, props: &RunProps, s: &str) {
        let mut out = String::new();
        for c in s.chars() {
            if matches!(c, ' ' | '\t' | '\n' | '\r') {
                if !self.last_space {
                    out.push(' ');
                    self.last_space = true;
                }
            } else {
                out.push(c);
                self.last_space = false;
            }
        }
        if !out.is_empty() {
            self.push(props, RunContent::Text(out));
        }
    }

    /// `text:s` spaces are never collapsed, nor do they collapse what follows.
    fn literal(&mut self, props: &RunProps, s: &str) {
        self.last_space = false;
        self.push(props, RunContent::Text(s.to_owned()));
    }

    fn finish(mut self) -> Vec<Run> {
        // Trailing collapsed white space is insignificant.
        if let Some(last) = self.runs.last_mut() {
            if let Some(RunContent::Text(t)) = last.content.last_mut() {
                if self.last_space && t.ends_with(' ') {
                    t.pop();
                }
                if t.is_empty() {
                    last.content.pop();
                }
            }
            if last.content.is_empty() {
                self.runs.pop();
            }
        }
        self.runs
    }
}

struct Reader<'a> {
    styles: &'a Styles,
    sections: Vec<Section>,
}

impl Reader<'_> {
    fn inline(&self, el: &Element, props: &RunProps, b: &mut RunBuilder, depth: usize) {
        if depth > dom::MAX_DEPTH {
            return;
        }
        for n in &el.children {
            match n {
                Node::Text(t) => b.text(props, t),
                Node::Element(e) => {
                    if e.ns == TEXT {
                        match e.name.as_str() {
                            // §6.1.3 text:s
                            "s" => {
                                let c: usize = e
                                    .attr(TEXT, "c")
                                    .and_then(|c| c.parse().ok())
                                    .unwrap_or(1)
                                    .min(10_000);
                                b.literal(props, &" ".repeat(c.max(1)));
                            }
                            // §6.1.4 text:tab, §6.1.5 text:line-break
                            "tab" => {
                                b.last_space = false;
                                b.push(props, RunContent::Tab);
                            }
                            "line-break" => {
                                b.last_space = true;
                                b.push(props, RunContent::LineBreak);
                            }
                            // §6.1.7 text:span
                            "span" => {
                                let own = self.styles.resolve("text", e.attr(TEXT, "style-name"));
                                let p = own.text.inherit(props);
                                self.inline(e, &p, b, depth + 1);
                            }
                            // Notes, annotations, bookmarks, change marks: no body text.
                            "note"
                            | "bookmark"
                            | "bookmark-start"
                            | "bookmark-end"
                            | "reference-mark"
                            | "reference-mark-start"
                            | "reference-mark-end"
                            | "change"
                            | "change-start"
                            | "change-end"
                            | "soft-page-break"
                            | "toc-mark"
                            | "alphabetical-index-mark" => {}
                            // text:a, fields (text:date, text:page-number, ...) show their content.
                            _ => self.inline(e, props, b, depth + 1),
                        }
                    } else if e.is(OFFICE, "annotation") || e.is(OFFICE, "annotation-end") {
                        // comments are not body text
                    } else if e.ns == crate::ns::DRAW {
                        // Frames anchored in the paragraph: text boxes are skipped for now.
                    } else {
                        self.inline(e, props, b, depth + 1);
                    }
                }
            }
        }
    }

    fn paragraph(&mut self, el: &Element) {
        let style = self
            .styles
            .resolve("paragraph", el.attr(TEXT, "style-name"));
        let props = style.para;
        if let Some(master) = &style.master_page {
            let has_content = self.sections.iter().any(|s| !s.blocks.is_empty());
            if let Some(page) = self.styles.page(master) {
                if has_content {
                    self.sections.push(Section {
                        page,
                        blocks: Vec::new(),
                    });
                } else if let Some(first) = self.sections.first_mut() {
                    first.page = page;
                }
            }
        }
        let mut b = RunBuilder {
            runs: Vec::new(),
            last_space: true,
        };
        self.inline(el, &style.text, &mut b, 0);
        let mut runs = b.finish();
        if style.break_after {
            // §20.183 fo:break-after="page" ↔ a trailing page break in the model.
            runs.push(Run {
                props: RunProps::default(),
                content: vec![RunContent::PageBreak],
            });
        }
        if let Some(sec) = self.sections.last_mut() {
            sec.blocks.push(Block::Paragraph(Paragraph { props, runs }));
        }
    }

    fn block(&mut self, el: &Element, depth: usize) {
        if depth > dom::MAX_DEPTH {
            return;
        }
        for e in el.elements() {
            match (e.ns.as_str(), e.name.as_str()) {
                // §5.1.3 text:p, §5.1.2 text:h
                (TEXT, "p" | "h") => self.paragraph(e),
                // Lists, sections, indexes, tables: descend into their paragraphs.
                (
                    TEXT,
                    "list" | "list-item" | "list-header" | "section" | "index-body"
                    | "table-of-content" | "alphabetical-index" | "illustration-index"
                    | "table-index" | "object-index" | "user-index" | "bibliography"
                    | "index-title",
                )
                | (
                    TABLE,
                    "table" | "table-row" | "table-cell" | "table-rows" | "table-row-group"
                    | "table-header-rows",
                ) => self.block(e, depth + 1),
                _ => {}
            }
        }
    }
}

/// Read an `.odt` package.
pub fn read_odt(bytes: &[u8]) -> Result<Document> {
    let pkg = OdfPackage::read(bytes)?;
    pkg.expect_mimetype(MIME_ODT)?;
    let content = dom::parse(pkg.require("content.xml")?, "content.xml")?;
    let styles_root = match pkg.file("styles.xml") {
        Some(s) => Some(dom::parse(s, "styles.xml")?),
        None => None,
    };
    let mut styles = Styles::default();
    if let Some(s) = &styles_root {
        styles.collect_fonts(s);
    }
    styles.collect_fonts(&content);
    if let Some(s) = &styles_root {
        if let Some(c) = s.child(OFFICE, "styles") {
            styles.collect(c);
        }
        if let Some(c) = s.child(OFFICE, "automatic-styles") {
            styles.collect(c);
        }
        styles.collect_masters(s);
    }
    if let Some(c) = content.child(OFFICE, "automatic-styles") {
        styles.collect(c);
    }
    let text = content
        .child(OFFICE, "body")
        .and_then(|b| b.child(OFFICE, "text"))
        .ok_or_else(|| OdfError::Missing("office:body/office:text".into()))?;
    let mut reader = Reader {
        styles: &styles,
        sections: vec![Section {
            page: styles.default_page(),
            blocks: Vec::new(),
        }],
    };
    reader.block(text, 0);
    Ok(Document {
        sections: reader.sections,
        default_run: styles.default_text.clone(),
        default_para: styles.default_para,
    })
}
