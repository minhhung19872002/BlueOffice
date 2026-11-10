//! PDF export choices (CORE-FMT-002): PDF/A-1b/2b/3b (ISO 19005), tagged PDF
//! (ISO 32000-1 §14.7–14.8) and bookmarks (§12.3.3) for BlueDoc, BlueSlide and
//! BlueSheet.
//!
//! - BlueDoc: the structure follows the document model — headings `H1`…`H6`
//!   (outline level, ECMA-376 Part 1 §17.3.1.20), paragraphs `P`, numbered and
//!   bulleted paragraphs (§17.9 numbering) as `L`/`LI`/`LBody`, tables
//!   `Table`/`TR`/`TH` (header rows, §17.4.49 tblHeader)/`TD`, pictures `Figure`
//!   with `/Alt` from `wp:docPr/@descr` (§20.4.2.5), text boxes and notes; page
//!   headers and footers are artifacts. Bookmarks come from the headings.
//! - BlueSlide: one `Sect` per slide, glyph runs on nearby baselines as `P`
//!   (the run matching the slide title as `H1`), pictures as `Figure` with the
//!   alt text of the slide's pictures; bookmarks from slide titles.
//! - BlueSheet: one `Sect` per sheet with a `Table` of the printed cell texts
//!   (rows by baseline, cells left to right); bookmarks from sheet names.

use crate::{Engine, EngineError, Result, SheetEngine};
use bo_layout::LineSource;
use bo_model::{Block, Document, RunContent};
use bo_pdf::{
    Attachment, Bookmark, ExportChoices, PdfA, PdfMeta, PdfOptions, Role, StructKid, StructNode,
};
use bo_render::{CommandSpan, DisplayList, PageList, SpanSource};
use std::collections::{BTreeMap, HashMap};

/// MIME types of the source packages attached to PDF/A-3 files (ECMA-376 Part 1, §11.3 / §12.3 / §13.3 content types).
const DOCX_MIME: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
const PPTX_MIME: &str = "application/vnd.openxmlformats-officedocument.presentationml.presentation";
const XLSX_MIME: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";

/// File name without its extension.
fn stem(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    base.rsplit_once('.')
        .map_or(base.to_owned(), |(b, _)| b.to_owned())
}

fn attachment(name: &str, ext: &str, mime: &str, data: Vec<u8>) -> Attachment {
    Attachment {
        name: format!("{}.{ext}", stem(name)),
        mime: mime.to_owned(),
        description: "Source file".to_owned(),
        data,
        source: true,
    }
}

/// Language of a document: its default paragraph style's, else the defaults
/// (ECMA-376 Part 1, §17.3.2.20 lang; §17.7.5.1 docDefaults).
fn doc_lang(doc: &Document) -> Option<String> {
    let from_style = doc
        .styles
        .default_of(bo_model::StyleKind::Paragraph)
        .and_then(|s| doc.styles.resolve_paragraph(&s.id).1.lang)
        .and_then(|l| l.val);
    from_style
        .or_else(|| doc.default_run.lang.as_ref().and_then(|l| l.val.clone()))
        .filter(|l| !l.trim().is_empty())
}

/// Content of one page: command ranges by source.
#[derive(Default)]
struct Contents {
    /// Body paragraph id → (page, start, end).
    body: HashMap<u32, Vec<(usize, usize, usize)>>,
    /// Text box (para, at, sub) → ranges.
    boxes: BTreeMap<(u32, u32, u32), Vec<(usize, usize, usize)>>,
    /// Note (endnote, id) → paragraph → ranges.
    notes: BTreeMap<(bool, i32), BTreeMap<u32, Vec<(usize, usize, usize)>>>,
    /// Pictures matched to body paragraphs: para → (alt, page, start, end).
    figures: HashMap<u32, Vec<(Option<String>, usize, usize, usize)>>,
}

/// Pictures of each body paragraph (display id, alt text), in paragraph-id order.
fn pictures(doc: &Document) -> Vec<(u32, String, Option<String>)> {
    fn walk(blocks: &[Block], next: &mut u32, out: &mut Vec<(u32, String, Option<String>)>) {
        for b in blocks {
            match b {
                Block::Paragraph(p) => {
                    let id = *next;
                    *next += 1;
                    for r in &p.runs {
                        for c in &r.content {
                            if let RunContent::Image(img) = c {
                                if img.shape.is_none() && !img.media.is_empty() {
                                    let alt = Some(img.description.trim().to_owned())
                                        .filter(|d| !d.is_empty());
                                    out.push((id, img.display_id(), alt));
                                }
                            }
                        }
                    }
                }
                Block::Table(t) => {
                    for row in &t.rows {
                        for cell in &row.cells {
                            walk(&cell.blocks, next, out);
                        }
                    }
                }
                Block::Opaque(_) => {}
            }
        }
    }
    let mut out = Vec::new();
    let mut next = 0;
    for s in &doc.sections {
        walk(&s.blocks, &mut next, &mut out);
    }
    out
}

fn collect(doc: &Document, spans: &[Vec<CommandSpan>]) -> Contents {
    let mut c = Contents::default();
    let mut pics = pictures(doc);
    let mut used = vec![false; pics.len()];
    for (page, list) in spans.iter().enumerate() {
        for s in list {
            let r = (page, s.start, s.end);
            match &s.source {
                SpanSource::Line(LineSource::Body(id)) => c.body.entry(*id).or_default().push(r),
                SpanSource::Line(LineSource::TextBox { para, at, sub })
                    if *para != LineSource::NO_PARA =>
                {
                    c.boxes.entry((*para, *at, *sub)).or_default().push(r);
                }
                SpanSource::Line(LineSource::Note { endnote, id, para }) => c
                    .notes
                    .entry((*endnote, *id))
                    .or_default()
                    .entry(*para)
                    .or_default()
                    .push(r),
                SpanSource::Image(media) => {
                    // The first unclaimed picture of the body with this id.
                    if let Some(i) = (0..pics.len()).find(|&i| !used[i] && pics[i].1 == *media) {
                        used[i] = true;
                        let alt = pics[i].2.take();
                        c.figures
                            .entry(pics[i].0)
                            .or_default()
                            .push((alt, page, s.start, s.end));
                    }
                }
                // Headers, footers and repeated header rows are artifacts.
                SpanSource::Line(_) => {}
            }
        }
    }
    c
}

fn with_content(role: Role, ranges: &[(usize, usize, usize)]) -> StructNode {
    let mut n = StructNode::new(role);
    for &(p, s, e) in ranges {
        n.push_content(p, s, e);
    }
    n
}

/// Open lists while walking the blocks: (level, `L` element).
type ListStack = Vec<(u8, StructNode)>;

/// Close the lists deeper than `level` (all when `None`), nesting each closed
/// list into the last item of its parent list.
fn close_lists(stack: &mut ListStack, level: Option<u8>, out: &mut Vec<StructNode>) {
    while let Some((l, _)) = stack.last() {
        if level.is_some_and(|k| *l <= k) {
            break;
        }
        let Some((_, list)) = stack.pop() else { break };
        match stack.last_mut() {
            Some((_, parent)) => match parent.kids.last_mut() {
                Some(StructKid::Node(li)) if li.role == Role::LI => {
                    li.kids.push(StructKid::Node(list));
                }
                _ => parent.kids.push(StructKid::Node(list)),
            },
            None => out.push(list),
        }
    }
}

struct DocTree<'a> {
    doc: &'a Document,
    contents: Contents,
    next: u32,
}

impl DocTree<'_> {
    fn blocks(&mut self, blocks: &[Block], out: &mut Vec<StructNode>) {
        let mut lists: ListStack = Vec::new();
        for b in blocks {
            match b {
                Block::Paragraph(p) => {
                    let id = self.next;
                    self.next += 1;
                    let level = bo_edit::fields::outline_level(self.doc, p);
                    let num = bo_layout::para_props(self.doc, p).num;
                    let text = self.contents.body.remove(&id).unwrap_or_default();
                    let figures = self.contents.figures.remove(&id).unwrap_or_default();
                    let mut nodes = Vec::new();
                    let role = match level {
                        Some(l) => Role::H(l.saturating_add(1).min(6)),
                        None if num.is_some() => Role::LBody,
                        None => Role::P,
                    };
                    let mut node = with_content(role, &text);
                    for (alt, page, s, e) in figures {
                        let mut f = with_content(Role::Figure, &[(page, s, e)]);
                        f.alt = alt;
                        node.kids.push(StructKid::Node(f));
                    }
                    // A paragraph holding only pictures is its figures.
                    if text.is_empty() && role == Role::P {
                        nodes.extend(node.kids.drain(..).filter_map(|k| match k {
                            StructKid::Node(n) => Some(n),
                            StructKid::Content { .. } => None,
                        }));
                    } else {
                        nodes.push(node);
                    }
                    // Text boxes anchored in the paragraph follow it.
                    let boxes: Vec<(u32, u32, u32)> = self
                        .contents
                        .boxes
                        .range((id, 0, 0)..=(id, u32::MAX, u32::MAX))
                        .map(|(k, _)| *k)
                        .collect();
                    for k in boxes {
                        if let Some(r) = self.contents.boxes.remove(&k) {
                            nodes.push(with_content(Role::P, &r));
                        }
                    }
                    match (role, num) {
                        (Role::LBody, Some(n)) => {
                            close_lists(&mut lists, Some(n.ilvl), out);
                            if lists.last().is_none_or(|(l, _)| *l < n.ilvl) {
                                lists.push((n.ilvl, StructNode::new(Role::L)));
                            }
                            let mut li = StructNode::new(Role::LI);
                            li.kids.extend(nodes.into_iter().map(StructKid::Node));
                            if let Some((_, l)) = lists.last_mut() {
                                l.kids.push(StructKid::Node(li));
                            }
                        }
                        _ => {
                            close_lists(&mut lists, None, out);
                            out.extend(nodes);
                        }
                    }
                }
                Block::Table(t) => {
                    close_lists(&mut lists, None, out);
                    let mut table = StructNode::new(Role::Table);
                    for row in &t.rows {
                        let mut tr = StructNode::new(Role::TR);
                        let role = if row.props.header { Role::TH } else { Role::TD };
                        for cell in &row.cells {
                            let mut kids = Vec::new();
                            self.blocks(&cell.blocks, &mut kids);
                            let mut td = StructNode::new(role);
                            td.kids.extend(kids.into_iter().map(StructKid::Node));
                            tr.kids.push(StructKid::Node(td));
                        }
                        table.kids.push(StructKid::Node(tr));
                    }
                    out.push(table);
                }
                Block::Opaque(_) => {}
            }
        }
        close_lists(&mut lists, None, out);
    }
}

/// Logical structure of a laid-out document (children of `/Document`).
pub(crate) fn doc_structure(doc: &Document, spans: &[Vec<CommandSpan>]) -> Vec<StructNode> {
    let mut t = DocTree {
        doc,
        contents: collect(doc, spans),
        next: 0,
    };
    let mut out = Vec::new();
    for s in &doc.sections {
        t.blocks(&s.blocks, &mut out);
    }
    // Footnotes and endnotes after the body (§14.8.4.2 Note).
    for (_, paras) in std::mem::take(&mut t.contents.notes) {
        let mut note = StructNode::new(Role::Note);
        for (_, r) in paras {
            note.kids.push(StructKid::Node(with_content(Role::P, &r)));
        }
        out.push(note);
    }
    out
}

/// Text drawn by the commands of a content range.
fn range_text(page: &PageList, start: usize, end: usize) -> String {
    page.commands
        .get(start..end)
        .unwrap_or_default()
        .iter()
        .filter_map(|c| match c {
            bo_render::Command::Glyphs { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn node_text(page: &PageList, n: &StructNode) -> String {
    n.kids
        .iter()
        .filter_map(|k| match k {
            StructKid::Content { start, end, .. } => Some(range_text(page, *start, *end)),
            StructKid::Node(_) => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Alt texts of a slide's pictures in z-order (groups descended).
fn slide_alts(nodes: &[bo_model::slide::ShapeNode], out: &mut Vec<Option<String>>) {
    use bo_model::slide::ShapeNode;
    for n in nodes {
        match n {
            ShapeNode::Picture(p) => out.push(
                p.nv.descr
                    .as_ref()
                    .map(|d| d.trim().to_owned())
                    .filter(|d| !d.is_empty()),
            ),
            ShapeNode::Group(g) => slide_alts(&g.children, out),
            _ => {}
        }
    }
}

impl Engine {
    /// Export the open document or presentation as PDF with `choices`
    /// (PDF/A part, tagged PDF, bookmarks).
    pub fn export_pdf_with(&self, choices: &ExportChoices) -> Result<Vec<u8>> {
        if self.session.is_some() {
            self.export_doc_pdf(choices)
        } else if self.pres.is_some() {
            self.export_slides_pdf(choices)
        } else {
            Err(EngineError::NoDocument)
        }
    }

    fn base_options(&self, choices: &ExportChoices, creator: &str) -> PdfOptions {
        PdfOptions {
            pdfa: choices.pdfa,
            creator: Some(creator.to_owned()),
            ..PdfOptions::default()
        }
    }

    fn export_doc_pdf(&self, choices: &ExportChoices) -> Result<Vec<u8>> {
        let session = self.session.as_ref().ok_or(EngineError::NoDocument)?;
        let doc = session.doc();
        let layout = self.layout.document();
        let mut pages = Vec::with_capacity(layout.pages.len());
        let mut spans = Vec::with_capacity(layout.pages.len());
        for p in &layout.pages {
            let (list, s) = bo_render::build_page_list_spans(p, &self.fonts);
            pages.push(list);
            spans.push(s);
        }
        let list = DisplayList { pages };
        let mut opts = self.base_options(choices, "BlueDoc");
        let props = doc
            .source
            .as_ref()
            .map(|src| crate::document_properties(&src.bytes, &self.name, src.bytes.len()));
        let mut title = stem(&self.name);
        if let Some(p) = &props {
            if let Some(t) = p.get("title") {
                title.clone_from(t);
            }
            opts.author = p.get("author").cloned();
        }
        opts.lang = doc_lang(doc);
        if choices.tagged {
            opts.structure = Some(doc_structure(doc, &spans));
        }
        if choices.bookmarks {
            // Position of each body paragraph's first line.
            let mut first: HashMap<usize, (usize, f32)> = HashMap::new();
            for (pi, p) in layout.pages.iter().enumerate() {
                for l in &p.lines {
                    if let LineSource::Body(id) = l.source {
                        first.entry(id as usize).or_insert((pi, l.top));
                    }
                }
            }
            opts.bookmarks = session
                .outline()
                .into_iter()
                .filter_map(|e| {
                    let level = e.level?;
                    let (page, y) = *first.get(&e.para)?;
                    (!e.text.is_empty()).then(|| Bookmark {
                        title: e.text,
                        level: level + 1,
                        page,
                        y,
                    })
                })
                .collect();
        }
        if choices.pdfa == Some(PdfA::A3b) {
            opts.attachments
                .push(attachment(&self.name, "docx", DOCX_MIME, self.save_docx()?));
        }
        Ok(bo_pdf::write_pdf_with_options(
            &list,
            &self.fonts,
            &PdfMeta { title: Some(title) },
            &self.image_source(),
            &opts,
        )?)
    }

    fn export_slides_pdf(&self, choices: &ExportChoices) -> Result<Vec<u8>> {
        let list = self.display.as_ref().ok_or(EngineError::NoDocument)?;
        let pres = self.pres.as_ref().ok_or(EngineError::NoDocument)?;
        let mut opts = self.base_options(choices, "BlueSlide");
        let titles: Vec<String> = pres
            .slides
            .iter()
            .map(|s| {
                crate::slide_edit::outline::slide_outline(s)
                    .title
                    .lines()
                    .map(str::trim)
                    .filter(|l| !l.is_empty())
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();
        if choices.tagged {
            let mut tree = Vec::new();
            for (i, page) in list.pages.iter().enumerate() {
                let mut nodes = bo_pdf::auto_paragraphs(page, i);
                let title = titles.get(i).map(|t| normalize(t)).unwrap_or_default();
                if !title.is_empty() {
                    if let Some(n) = nodes
                        .iter_mut()
                        .find(|n| n.role == Role::P && normalize(&node_text(page, n)) == title)
                    {
                        n.role = Role::H(1);
                    }
                }
                // Alt text: the slide's own pictures are drawn last, in z-order.
                let mut alts = Vec::new();
                if let Some(s) = pres.slides.get(i) {
                    slide_alts(&s.data.shapes, &mut alts);
                }
                let figures = nodes.iter().filter(|n| n.role == Role::Figure).count();
                if figures >= alts.len() {
                    let skip = figures - alts.len();
                    for (n, alt) in nodes
                        .iter_mut()
                        .filter(|n| n.role == Role::Figure)
                        .skip(skip)
                        .zip(alts)
                    {
                        n.alt = alt;
                    }
                }
                let mut sect = StructNode::new(Role::Sect);
                sect.kids.extend(nodes.into_iter().map(StructKid::Node));
                tree.push(sect);
            }
            opts.structure = Some(tree);
        }
        if choices.bookmarks {
            opts.bookmarks = titles
                .iter()
                .enumerate()
                .filter(|(i, _)| *i < list.pages.len())
                .map(|(i, t)| Bookmark {
                    title: if t.is_empty() {
                        format!("{}", i + 1)
                    } else {
                        t.clone()
                    },
                    level: 1,
                    page: i,
                    y: 0.0,
                })
                .collect();
        }
        if choices.pdfa == Some(PdfA::A3b) {
            opts.attachments
                .push(attachment(&self.name, "pptx", PPTX_MIME, self.save_pptx()?));
        }
        Ok(bo_pdf::write_pdf_with_options(
            list,
            &self.fonts,
            &PdfMeta {
                title: Some(stem(&self.name)),
            },
            &self.image_source(),
            &opts,
        )?)
    }
}

impl SheetEngine {
    /// PDF of every visible sheet with `choices` (PDF/A part, tagged PDF,
    /// bookmarks from sheet names).
    pub fn export_pdf_with(&mut self, choices: &ExportChoices) -> Result<Vec<u8>> {
        let (pages, firsts) = self.printed_pages()?;
        let list = DisplayList { pages };
        let mut opts = PdfOptions {
            pdfa: choices.pdfa,
            creator: Some("BlueSheet".to_owned()),
            ..PdfOptions::default()
        };
        if choices.tagged {
            let mut tree = Vec::new();
            for (k, (_, first)) in firsts.iter().enumerate() {
                let end = firsts.get(k + 1).map_or(list.pages.len(), |(_, f)| *f);
                let mut sect = StructNode::new(Role::Sect);
                for i in *first..end {
                    if let Some(p) = list.pages.get(i) {
                        sect.kids
                            .extend(bo_pdf::auto_grid(p, i).into_iter().map(StructKid::Node));
                    }
                }
                tree.push(sect);
            }
            opts.structure = Some(tree);
        }
        if choices.bookmarks {
            opts.bookmarks = firsts
                .iter()
                .map(|(name, page)| Bookmark {
                    title: name.clone(),
                    level: 1,
                    page: *page,
                    y: 0.0,
                })
                .collect();
        }
        if choices.pdfa == Some(PdfA::A3b) {
            let name = self.name.clone();
            opts.attachments
                .push(attachment(&name, "xlsx", XLSX_MIME, self.save("xlsx", 0)?));
        }
        Ok(bo_pdf::write_pdf_with_options(
            &list,
            &self.fonts,
            &PdfMeta {
                title: Some(stem(&self.name)),
            },
            &BTreeMap::<String, Vec<u8>>::new(),
            &opts,
        )?)
    }
}
