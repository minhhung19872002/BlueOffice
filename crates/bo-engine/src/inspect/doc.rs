//! BlueDoc checks over the WordprocessingML model (ECMA-376 Part 1, §17):
//! inspector categories, accessibility rules and compatibility features, plus
//! the model-level removals and quick fixes.

use super::a11y;
use super::compat::{self, BOTH, LEGACY, ODF};
use super::package::{self, PackageScan, Scrub};
use super::{
    looks_like_url, sample, Action, Category, Feature, Finding, FixKind, Issue, Report, Rule,
    Severity, Target,
};
use bo_model::index::para_index;
use bo_model::{Block, Color, Comment, Document, Image, Paragraph, RunContent, Section};
use std::collections::BTreeMap;

/// Documents with at least this many words should have headings.
const LONG_DOCUMENT_WORDS: usize = 1000;

/// Every block list of the document that holds body-like content: the sections'
/// headers and footers (`hf = true`) and their bodies.
fn hf_blocks(s: &Section) -> impl Iterator<Item = &Vec<Block>> {
    [
        &s.headers.default,
        &s.headers.first,
        &s.headers.even,
        &s.footers.default,
        &s.footers.first,
        &s.footers.even,
    ]
    .into_iter()
    .filter_map(Option::as_ref)
}

/// Visit every paragraph of `blocks`, descending into table cells and text boxes.
fn visit(blocks: &[Block], f: &mut dyn FnMut(&Paragraph)) {
    for b in blocks {
        match b {
            Block::Paragraph(p) => {
                f(p);
                for r in &p.runs {
                    for c in &r.content {
                        if let RunContent::Image(img) = c {
                            if let Some(t) = img.shape.as_ref().and_then(|s| s.text.as_ref()) {
                                visit(t, f);
                            }
                        }
                    }
                }
            }
            Block::Table(t) => {
                for row in &t.rows {
                    for cell in &row.cells {
                        visit(&cell.blocks, f);
                    }
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

/// Mutable [`visit`].
fn visit_mut(blocks: &mut [Block], f: &mut dyn FnMut(&mut Paragraph)) {
    for b in blocks {
        match b {
            Block::Paragraph(p) => {
                f(p);
                for r in &mut p.runs {
                    for c in &mut r.content {
                        if let RunContent::Image(img) = c {
                            if let Some(t) = img.shape.as_mut().and_then(|s| s.text.as_mut()) {
                                visit_mut(t, f);
                            }
                        }
                    }
                }
            }
            Block::Table(t) => {
                for row in &mut t.rows {
                    for cell in &mut row.cells {
                        visit_mut(&mut cell.blocks, f);
                    }
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

/// Every paragraph: the body, headers / footers (`true`) and the footnotes and endnotes
/// (§17.11).
fn all_paragraphs(doc: &Document, f: &mut dyn FnMut(&Paragraph, bool)) {
    for s in &doc.sections {
        visit(&s.blocks, &mut |p| f(p, false));
        for b in hf_blocks(s) {
            visit(b, &mut |p| f(p, true));
        }
    }
    for b in doc.footnotes.values().chain(doc.endnotes.values()) {
        visit(b, &mut |p| f(p, false));
    }
}

fn raw_of(c: &RunContent) -> Option<&str> {
    match c {
        RunContent::Opaque(r) | RunContent::Markup(r) => Some(&r.xml),
        RunContent::Image(img) => img.src.as_ref().map(|r| &*r.xml),
        _ => None,
    }
}

/// A watermark: the text / picture shape a header carries behind the page.
fn is_watermark(c: &RunContent) -> bool {
    raw_of(c).is_some_and(|x| x.to_ascii_lowercase().contains("watermark"))
}

/// A drawing marked hidden (§20.4.2.5 docPr `hidden`).
fn is_invisible(c: &RunContent) -> bool {
    matches!(c, RunContent::Image(_))
        && raw_of(c)
            .and_then(|x| package::first_attr(x, "docPr", "hidden"))
            .is_some_and(|v| v == "1" || v == "true")
}

fn is_text(c: &RunContent) -> bool {
    matches!(
        c,
        RunContent::Text(_)
            | RunContent::Tab
            | RunContent::LineBreak
            | RunContent::SoftHyphen
            | RunContent::NoBreakHyphen
    )
}

/// The run's effective `w:vanish` (§17.3.2.41) with style inheritance.
fn hidden(doc: &Document, p: &Paragraph, r: &bo_model::Run) -> bool {
    r.props.hidden == Some(true)
        || (r.props.hidden.is_none()
            && bo_edit::effective_run(doc, p, &r.props).hidden == Some(true))
}

fn run_text(r: &bo_model::Run) -> String {
    let mut s = String::new();
    for c in &r.content {
        if is_text(c) {
            c.push_text(&mut s);
        }
    }
    s
}

/// Inspector, accessibility and compatibility report of a document.
pub fn report(doc: &Document, pkg: &PackageScan, pending: &Scrub) -> Report {
    Report::new(
        inspector(doc, pkg, pending),
        accessibility(doc, pkg),
        compatibility(doc, pkg),
        pending,
    )
}

fn package_findings(pkg: &PackageScan) -> Vec<Finding> {
    vec![
        Finding::new(Category::Properties, pkg.properties.len(), Action::OnSave).with_details(
            pkg.properties
                .iter()
                .map(|(k, v)| format!("{k}: {}", sample(v))),
        ),
        Finding::new(
            Category::CustomProperties,
            pkg.custom_properties,
            Action::OnSave,
        )
        .with_details(pkg.custom_names.iter().cloned()),
        Finding::new(Category::CustomXml, pkg.custom_xml, Action::OnSave),
        Finding::new(
            Category::ServerProperties,
            pkg.server_properties,
            Action::OnSave,
        ),
        Finding::new(Category::Macros, pkg.macros, Action::OnSave),
    ]
}

fn inspector(doc: &Document, pkg: &PackageScan, pending: &Scrub) -> Vec<Finding> {
    let mut hidden_runs = 0;
    let mut hidden_samples = Vec::new();
    let mut watermarks = 0;
    let mut invisible = 0;
    let mut embedded = 0;
    all_paragraphs(doc, &mut |p, in_hf| {
        for r in &p.runs {
            let t = run_text(r);
            if !t.trim().is_empty() && hidden(doc, p, r) {
                hidden_runs += 1;
                hidden_samples.push(sample(&t));
            }
            for c in &r.content {
                if in_hf && is_watermark(c) {
                    watermarks += 1;
                }
                if is_invisible(c) {
                    invisible += 1;
                }
                if is_embedded(c) {
                    embedded += 1;
                }
            }
        }
    });
    let hf: usize = doc
        .sections
        .iter()
        .map(|s| hf_blocks(s).filter(|b| has_content(b)).count())
        .sum();
    let revisions = bo_edit::revisions::list(doc).len();
    let mut out = vec![
        Finding::new(Category::Comments, doc.comments.len(), Action::Remove)
            .with_details(doc.comments.iter().map(|c| c.author.clone())),
        Finding::new(Category::Revisions, revisions, Action::AcceptAll),
    ];
    out.extend(package_findings(pkg));
    out.extend([
        Finding::new(Category::HiddenText, hidden_runs, Action::Remove)
            .with_details(hidden_samples),
        Finding::new(Category::HeadersFooters, hf, Action::Remove),
        Finding::new(Category::Watermarks, watermarks, Action::Remove),
        Finding::new(Category::InvisibleObjects, invisible, Action::Remove),
        // Embedded objects (§17.3.3.19 object): removed from the model, each keeps its
        // picture; other embedded parts of the package are only reported.
        if embedded > 0 {
            Finding::new(Category::Embedded, embedded, Action::Remove)
                .with_details(pkg.embedded_names.iter().cloned())
        } else {
            // Parts the model has no object for are only reported; once the objects were
            // removed (queued), their parts go with the next save.
            let left = if pending.has(Category::Embedded) {
                0
            } else {
                pkg.embedded
            };
            Finding::new(Category::Embedded, left, Action::None)
                .with_details(pkg.embedded_names.iter().cloned())
        },
    ]);
    out
}

/// An embedded (not linked) OLE object (§17.3.3.19 object with `o:OLEObject Type="Embed"`).
fn is_embedded(c: &RunContent) -> bool {
    matches!(c, RunContent::Image(img) if img.ole.as_ref().is_some_and(|o| o.link.is_none()))
}

/// The blocks hold text or objects.
fn has_content(blocks: &[Block]) -> bool {
    let mut any = false;
    visit(blocks, &mut |p| {
        any |= p.runs.iter().any(|r| {
            r.content.iter().any(|c| {
                !is_text(c) && !matches!(c, RunContent::Markup(_))
                    || is_text(c) && !run_text(r).trim().is_empty()
            })
        });
    });
    any || blocks.iter().any(|b| matches!(b, Block::Opaque(_)))
}

/// Body walk with BlueDoc paragraph ids (the order of `body_paragraphs`).
struct Walk<'a> {
    doc: &'a Document,
    id: usize,
    issues: Vec<Issue>,
    last_heading: Option<u8>,
    headings: usize,
    words: usize,
}

impl<'a> Walk<'a> {
    fn blocks(&mut self, blocks: &'a [Block], bg: Option<Color>) {
        for b in blocks {
            match b {
                Block::Paragraph(p) => {
                    self.paragraph(p, bg);
                    self.id += 1;
                }
                Block::Table(t) => {
                    let first = self.id;
                    if t.rows.len() >= 2 && !t.rows[0].props.header {
                        self.issues.push(
                            Issue::new(
                                Rule::TableHeader,
                                Severity::Warning,
                                Target::Table { para: first },
                                table_sample(t),
                            )
                            .fix(FixKind::HeaderRow),
                        );
                    }
                    let grid: Vec<Vec<bool>> = t
                        .rows
                        .iter()
                        .map(|r| r.cells.iter().map(|c| has_content(&c.blocks)).collect())
                        .collect();
                    if a11y::blank_line(&grid) {
                        self.issues.push(Issue::new(
                            Rule::LayoutTable,
                            Severity::Tip,
                            Target::Table { para: first },
                            table_sample(t),
                        ));
                    }
                    for row in &t.rows {
                        for cell in &row.cells {
                            self.blocks(&cell.blocks, cell.props.shading.or(bg));
                        }
                    }
                }
                Block::Opaque(_) => {}
            }
        }
    }

    fn paragraph(&mut self, p: &'a Paragraph, cell_bg: Option<Color>) {
        let doc = self.doc;
        let text = p.plain_text();
        self.words += text.split_whitespace().count();
        let eff = bo_edit::effective_para(doc, &p.props);
        // §17.3.1.20 outlineLvl: 0–8 are heading levels, 9 is body text.
        if let Some(l) = eff.outline_level.filter(|l| *l < 9) {
            if !text.trim().is_empty() {
                if let Some(prev) = self.last_heading {
                    if l > prev + 1 {
                        self.issues.push(Issue::new(
                            Rule::HeadingOrder,
                            Severity::Warning,
                            Target::Paragraph { para: self.id },
                            sample(&text),
                        ));
                    }
                }
                self.last_heading = Some(l);
                self.headings += 1;
            }
        }
        let (_, spans) = para_index(p);
        for s in &spans {
            if let RunContent::Image(img) = &p.runs[s.run].content[s.item] {
                if needs_alt_text(img) {
                    let name = img
                        .src
                        .as_ref()
                        .and_then(|r| package::first_attr(&r.xml, "docPr", "name"))
                        .unwrap_or_default();
                    self.issues.push(
                        Issue::new(
                            Rule::AltText,
                            Severity::Error,
                            Target::Drawing {
                                para: self.id,
                                at: s.start,
                            },
                            name,
                        )
                        .fix(FixKind::AltText),
                    );
                }
            }
        }
        // Contrast: the first run of the paragraph below the minimum.
        let page_bg = eff.shading.or(cell_bg).unwrap_or(Color::WHITE);
        for r in &p.runs {
            let t = run_text(r);
            if t.trim().is_empty() || hidden(doc, p, r) {
                continue;
            }
            let e = bo_edit::effective_run(doc, p, &r.props);
            let Some(fg) = e.color else { continue }; // automatic colour adapts
            let bg = r.props.highlight.or(e.shading).unwrap_or(page_bg);
            let size = e.size.map_or(11.0, |h| f64::from(h.0) / 2.0);
            if let Some(ratio) = a11y::low_contrast(
                (fg.r, fg.g, fg.b),
                (bg.r, bg.g, bg.b),
                size,
                e.bold == Some(true),
            ) {
                let mut i = Issue::new(
                    Rule::Contrast,
                    Severity::Warning,
                    Target::Paragraph { para: self.id },
                    sample(&t),
                );
                i.ratio = Some(ratio);
                self.issues.push(i);
                break;
            }
        }
        // Links: consecutive runs with the same target are one link.
        let mut i = 0;
        while i < p.runs.len() {
            let Some(link) = p.runs[i].props.link.clone() else {
                i += 1;
                continue;
            };
            let mut t = String::new();
            while i < p.runs.len() && p.runs[i].props.link.as_deref() == Some(&link) {
                t.push_str(&run_text(&p.runs[i]));
                i += 1;
            }
            if !link.starts_with('#')
                && (t.trim().is_empty() || looks_like_url(&t) || t.trim() == link)
            {
                self.issues.push(Issue::new(
                    Rule::LinkText,
                    Severity::Warning,
                    Target::Paragraph { para: self.id },
                    sample(&link),
                ));
            }
        }
    }
}

fn table_sample(t: &bo_model::Table) -> String {
    let mut s = String::new();
    if let Some(row) = t.rows.first() {
        for c in &row.cells {
            visit(&c.blocks, &mut |p| {
                if s.len() < 80 {
                    s.push_str(&p.plain_text());
                    s.push(' ');
                }
            });
        }
    }
    sample(&s)
}

/// A picture, or a shape without text, that has no alternative text and is not
/// marked decorative ([MS-ODRAWXML] `adec:decorative`).
fn needs_alt_text(img: &Image) -> bool {
    if !img.description.trim().is_empty() {
        return false;
    }
    if let Some(s) = &img.shape {
        if s.text.as_ref().is_some_and(|b| has_content(b)) {
            return false;
        }
    }
    let decorative = img
        .src
        .as_ref()
        .and_then(|r| package::first_attr(&r.xml, "decorative", "val"))
        .is_some_and(|v| v == "1" || v == "true");
    !decorative
}

fn accessibility(doc: &Document, pkg: &PackageScan) -> Vec<Issue> {
    let mut w = Walk {
        doc,
        id: 0,
        issues: Vec::new(),
        last_heading: None,
        headings: 0,
        words: 0,
    };
    for s in &doc.sections {
        w.blocks(&s.blocks, None);
    }
    if w.headings == 0 && w.words >= LONG_DOCUMENT_WORDS {
        w.issues.push(Issue::new(
            Rule::NoHeadings,
            Severity::Tip,
            Target::Document,
            "",
        ));
    }
    if pkg.title.is_empty() {
        w.issues.push(Issue::new(
            Rule::DocumentTitle,
            Severity::Tip,
            Target::Document,
            "",
        ));
    }
    w.issues
}

/// Count the tables (nested ones too) and those wider than 63 columns.
fn tables_in(blocks: &[Block], n: &mut usize, wide: &mut usize) {
    for b in blocks {
        if let Block::Table(t) = b {
            *n += 1;
            if t.grid.len() > 63 {
                *wide += 1;
            }
            for r in &t.rows {
                for c in &r.cells {
                    tables_in(&c.blocks, n, wide);
                }
            }
        }
    }
}

fn compatibility(doc: &Document, pkg: &PackageScan) -> Vec<super::CompatItem> {
    let (mut controls, mut equations, mut drawings, mut effects, mut long_marks, mut fields) =
        (0, 0, 0, 0, 0, 0);
    all_paragraphs(doc, &mut |p, _| {
        for r in &p.runs {
            if r.props.unknown.iter().any(|u| u.xml.contains("w14:")) {
                effects += 1;
            }
            for c in &r.content {
                match c {
                    RunContent::ControlStart(_) => controls += 1,
                    RunContent::Image(_) => drawings += 1,
                    RunContent::Field { .. } => fields += 1,
                    RunContent::Markup(m) if bo_model::index::is_math_xml(&m.xml) => equations += 1,
                    RunContent::Markup(m)
                        if package::first_attr(&m.xml, "bookmarkStart", "w:name")
                            .is_some_and(|n| n.chars().count() > 40) =>
                    {
                        long_marks += 1;
                    }
                    _ => {}
                }
            }
        }
    });
    let (mut tables, mut wide) = (0, 0);
    for s in &doc.sections {
        tables_in(&s.blocks, &mut tables, &mut wide);
    }
    let hf: usize = doc.sections.iter().map(|s| hf_blocks(s).count()).sum();
    let notes = doc
        .footnotes
        .keys()
        .chain(doc.endnotes.keys())
        .filter(|k| **k > 0)
        .count();
    let threads = doc
        .comments
        .iter()
        .filter(|c| c.parent_para_id.is_some() || c.done)
        .count();
    vec![
        compat::item(Feature::ContentControls, controls, BOTH),
        compat::item(Feature::Equations, equations, BOTH),
        compat::item(Feature::SmartArt, pkg.diagrams, BOTH),
        compat::item(Feature::ModernCharts, pkg.chart_ex, BOTH),
        compat::item(Feature::Charts, pkg.charts, ODF),
        compat::item(Feature::Tables, tables, ODF),
        compat::item(Feature::WideTables, wide, LEGACY),
        compat::item(Feature::Drawings, drawings, ODF),
        compat::item(Feature::Comments, doc.comments.len(), ODF),
        compat::item(Feature::CommentThreads, threads, LEGACY),
        compat::item(
            Feature::TrackedChanges,
            bo_edit::revisions::list(doc).len(),
            ODF,
        ),
        compat::item(Feature::HeadersFooters, hf, ODF),
        compat::item(Feature::Notes, notes, ODF),
        compat::item(Feature::TextEffects, effects, BOTH),
        compat::item(Feature::LongBookmarks, long_marks, LEGACY),
        compat::item(Feature::Fields, fields, ODF),
    ]
}

/// Strip run content items for which `drop` is true (runs left empty go too).
fn strip(p: &mut Paragraph, drop: &mut dyn FnMut(&RunContent) -> bool) {
    let mut changed = false;
    for r in &mut p.runs {
        let before = r.content.len();
        r.content.retain(|c| !drop(c));
        if r.content.len() != before {
            changed = true;
        }
    }
    if changed {
        p.runs.retain(|r| !r.content.is_empty());
    }
}

/// The document content after the inspector removed something.
#[derive(Debug, Clone, PartialEq)]
pub struct Cleaned {
    /// Sections (§17.6).
    pub sections: Vec<Section>,
    /// Comments (§17.13.4).
    pub comments: Vec<Comment>,
    /// Footnotes by id (§17.11.10).
    pub footnotes: BTreeMap<i32, Vec<Block>>,
    /// Endnotes by id (§17.11.2).
    pub endnotes: BTreeMap<i32, Vec<Block>>,
}

/// The content with the model-level categories in `cats` removed (hidden text, comment
/// marks and invisible objects also inside footnotes and endnotes).
pub fn remove(doc: &Document, cats: &[Category]) -> Cleaned {
    let mut sections = doc.sections.clone();
    let mut comments = doc.comments.clone();
    let mut footnotes = doc.footnotes.clone();
    let mut endnotes = doc.endnotes.clone();
    let has = |c: Category| cats.contains(&c);
    let clean = |p: &mut Paragraph, in_hf: bool| {
        if has(Category::Comments) {
            strip(p, &mut |c| {
                matches!(
                    c,
                    RunContent::CommentStart(_)
                        | RunContent::CommentEnd(_)
                        | RunContent::CommentRef(_)
                )
            });
        }
        if has(Category::Watermarks) && in_hf {
            strip(p, &mut is_watermark);
        }
        if has(Category::InvisibleObjects) {
            strip(p, &mut is_invisible);
        }
        if has(Category::Embedded) {
            // The embedded document goes; its picture stays in its place.
            for r in &mut p.runs {
                for c in &mut r.content {
                    if is_embedded(c) {
                        if let RunContent::Image(img) = c {
                            img.ole = None;
                            img.src = None;
                        }
                    }
                }
            }
        }
        if has(Category::HiddenText) {
            let snapshot = p.clone();
            let mut changed = false;
            for (r, orig) in p.runs.iter_mut().zip(&snapshot.runs) {
                if hidden(doc, &snapshot, orig) && !run_text(orig).trim().is_empty() {
                    r.content.retain(|c| !is_text(c));
                    changed = true;
                }
            }
            if changed {
                p.runs.retain(|r| !r.content.is_empty());
            }
        }
    };
    for blocks in footnotes.values_mut().chain(endnotes.values_mut()) {
        visit_mut(blocks, &mut |p| clean(p, false));
    }
    for s in &mut sections {
        if has(Category::HeadersFooters) {
            s.headers = bo_model::HeaderFooter::default();
            s.footers = bo_model::HeaderFooter::default();
        }
        let mut lists: Vec<&mut Vec<Block>> = vec![&mut s.blocks];
        let hf_start = 1;
        for v in [
            &mut s.headers.default,
            &mut s.headers.first,
            &mut s.headers.even,
            &mut s.footers.default,
            &mut s.footers.first,
            &mut s.footers.even,
        ]
        .into_iter()
        .flatten()
        {
            lists.push(v);
        }
        for (li, blocks) in lists.into_iter().enumerate() {
            let in_hf = li >= hf_start;
            visit_mut(blocks, &mut |p| clean(p, in_hf));
        }
    }
    if has(Category::Comments) {
        comments.clear();
    }
    Cleaned {
        sections,
        comments,
        footnotes,
        endnotes,
    }
}

/// Sections with the first row of the table starting at paragraph `para` marked as
/// its header row (§17.4.49 tblHeader); `None` when no table starts there.
pub fn set_header_row(doc: &Document, para: usize) -> Option<Vec<Section>> {
    fn walk(blocks: &mut [Block], id: &mut usize, para: usize) -> bool {
        for b in blocks {
            match b {
                Block::Paragraph(_) => *id += 1,
                Block::Table(t) => {
                    if *id == para {
                        if let Some(r) = t.rows.first_mut() {
                            r.props.header = true;
                            return true;
                        }
                    }
                    for row in &mut t.rows {
                        for cell in &mut row.cells {
                            if walk(&mut cell.blocks, id, para) {
                                return true;
                            }
                        }
                    }
                }
                Block::Opaque(_) => {}
            }
        }
        false
    }
    let mut sections = doc.sections.clone();
    let mut id = 0;
    for s in &mut sections {
        if walk(&mut s.blocks, &mut id, para) {
            return Some(sections);
        }
    }
    None
}

/// Set the alternative text of a drawing (`wp:docPr/@descr`, §20.4.2.5), in the
/// model and in its kept source XML.
pub fn set_alt_text(img: &mut Image, text: &str) {
    text.clone_into(&mut img.description);
    if let Some(src) = img.src.as_mut() {
        let xml = package::set_first_attr(&src.xml, "docPr", "descr", text);
        src.xml = xml.into();
    }
}
