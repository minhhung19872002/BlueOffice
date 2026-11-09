//! `bo_model::Document` → `.docx` (ECMA-376 Part 1, §17).
//!
//! Lossless strategy (ADR-0005): when the document came from a package, the
//! package is copied and only `document.xml` (plus styles/numbering/headers when
//! their model changed) is rewritten. Inside `document.xml`, every block whose
//! fingerprint matches its source is emitted verbatim; changed blocks are
//! regenerated, keeping unknown property children in schema order.

use crate::{Result, CT_DOCUMENT, CT_STYLES};
use bo_model::{
    fingerprint, Alignment, Block, Border, Borders, Cell, CellVAlign, ContentControl, ControlKind,
    Document, HeightRule, Image, LineSpacing, ParaProps, Paragraph, Raw, RevisionInfo, Run,
    RunContent, RunProps, Section, SectionBreak, StyleKind, TabAlign, TabLeader, Table, TableProps,
    TableWidth, VMerge, VertAlign, Wrap, WrapText,
};
use bo_opc::xml::escape;
use bo_opc::{Limits, Package, Relationship, TargetMode, REL_OFFICE_DOCUMENT, REL_STYLES};
use std::fmt::Write as _;

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const WP_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
const A_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const PIC_NS: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";
const REL_IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
const REL_HYPERLINK: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/hyperlink";
const REL_HEADER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
const REL_FOOTER: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer";
const REL_NUMBERING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
const REL_SETTINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
const CT_NUMBERING: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml";
const CT_SETTINGS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml";
const CT_HEADER: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.header+xml";
const CT_FOOTER: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.footer+xml";
const REL_FOOTNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes";
const REL_ENDNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes";
const CT_FOOTNOTES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.footnotes+xml";
const CT_ENDNOTES: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.endnotes+xml";
const CT_COMMENTS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.comments+xml";
const CT_COMMENTS_EXT: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.commentsExtended+xml";
const CT_COMMENTS_IDS: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.commentsIds+xml";
const W14_NS: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
const W15_NS: &str = "http://schemas.microsoft.com/office/word/2012/wordml";
const W16CID_NS: &str = "http://schemas.microsoft.com/office/word/2016/wordml/cid";
const MC_NS: &str = "http://schemas.openxmlformats.org/markup-compatibility/2006";

/// Relationships of the main document part being written.
struct Rels {
    part: String,
    list: Vec<Relationship>,
    changed: bool,
}

impl Rels {
    fn id_for(&mut self, rel_type: &str, target: &str, external: bool) -> String {
        if let Some(r) = self.list.iter().find(|r| {
            r.rel_type == rel_type
                && (r.target == target
                    || (!external && bo_opc::resolve_target(&self.part, &r.target) == target))
        }) {
            return r.id.clone();
        }
        let mut n = self.list.len() + 1;
        while self.list.iter().any(|r| r.id == format!("rId{n}")) {
            n += 1;
        }
        let id = format!("rId{n}");
        self.list.push(Relationship {
            id: id.clone(),
            rel_type: rel_type.to_owned(),
            target: target.to_owned(),
            mode: if external {
                TargetMode::External
            } else {
                TargetMode::Internal
            },
        });
        self.changed = true;
        id
    }
}

struct Writer<'d> {
    doc: &'d Document,
    pkg: Package,
    rels: Rels,
    hf_counter: usize,
    drawing_id: u32,
    /// Writing endnote content (`NoteMark` becomes `w:endnoteRef`).
    in_endnote: bool,
    /// Inline content controls open in the paragraph being written.
    sdt_depth: usize,
}

/// Serialize a document to `.docx` bytes.
pub fn write_docx(doc: &Document) -> Result<Vec<u8>> {
    reset_ids();
    let (pkg, main) = match &doc.source {
        Some(src) => (
            Package::read(&src.bytes, &Limits::default())?,
            src.main_part.clone(),
        ),
        None => (Package::new(), "/word/document.xml".to_owned()),
    };
    let existing = pkg.relationships(&main)?;
    let mut w = Writer {
        doc,
        pkg,
        rels: Rels {
            part: main.clone(),
            list: existing,
            changed: false,
        },
        hf_counter: 0,
        drawing_id: 1000,
        in_endnote: false,
        sdt_depth: 0,
    };
    w.write_main(&main)?;
    w.write_styles(&main)?;
    w.write_numbering(&main)?;
    w.write_comments(&main)?;
    if let Some(src) = &doc.source {
        // Edited notes / header settings of a read document (§17.11, §17.15.1.35).
        if src.notes_fingerprint != notes_fingerprint(doc) {
            w.write_notes(&main)?;
        }
        if src.even_and_odd_headers != doc.even_and_odd_headers
            || src.display_background_shape != doc.design.display_background_shape
            || src.hyphenation != doc.hyphenation
        {
            w.patch_settings(&main)?;
        }
    }
    w.write_theme(&main)?;
    if doc.source.is_none() {
        w.write_settings(&main)?;
        w.write_notes(&main)?;
        w.pkg.set_relationships(
            "/",
            &[Relationship {
                id: "rId1".into(),
                rel_type: REL_OFFICE_DOCUMENT.into(),
                target: main.trim_start_matches('/').to_owned(),
                mode: TargetMode::Internal,
            }],
        );
    }
    w.write_media()?;
    if w.rels.changed || doc.source.is_none() {
        let rels = w.rels.list.clone();
        w.pkg.set_relationships(&main, &rels);
    }
    Ok(w.pkg.write()?)
}

/// Fingerprint of the footnotes and endnotes (detects edited notes).
pub(crate) fn notes_fingerprint(doc: &Document) -> u64 {
    fingerprint(&(&doc.footnotes, &doc.endnotes))
}

/// Byte offset of the first `<tag` start (followed by `/`, `>` or whitespace).
fn find_element(xml: &str, tag: &str) -> Option<usize> {
    let pat = format!("<{tag}");
    let mut from = 0;
    while let Some(i) = xml[from..].find(&pat) {
        let at = from + i;
        let next = xml[at + pat.len()..].chars().next();
        if next.is_some_and(|c| c == '/' || c == '>' || c.is_whitespace()) {
            return Some(at);
        }
        from = at + pat.len();
    }
    None
}

/// Remove every empty element `<tag …/>` (or `<tag …></tag>`).
/// CT_Settings children that follow the hyphenation settings (§17.15.1.78).
const AFTER_HYPHENATION: &[&str] = &[
    "showEnvelope",
    "summaryLength",
    "clickAndTypeStyle",
    "defaultTableStyle",
    "evenAndOddHeaders",
    "bookFoldRevPrinting",
    "bookFoldPrinting",
    "bookFoldPrintingSheets",
    "drawingGridHorizontalSpacing",
    "drawingGridVerticalSpacing",
    "displayHorizontalDrawingGridEvery",
    "displayVerticalDrawingGridEvery",
    "doNotUseMarginsForDrawingGridOrigin",
    "drawingGridHorizontalOrigin",
    "drawingGridVerticalOrigin",
    "doNotShadeFormData",
    "noPunctuationKerning",
    "characterSpacingControl",
    "printTwoOnOne",
    "strictFirstAndLastChars",
    "noLineBreaksAfter",
    "noLineBreaksBefore",
    "savePreviewPicture",
    "updateFields",
    "hdrShapeDefaults",
    "footnotePr",
    "endnotePr",
    "compat",
    "docVars",
    "rsids",
    "mathPr",
    "attachedSchema",
    "themeFontLang",
    "clrSchemeMapping",
    "doNotIncludeSubdocsInStats",
    "doNotAutoCompressPictures",
    "forceUpgrade",
    "captions",
    "readModeInkLockDown",
    "smartTagType",
    "schemaLibrary",
    "shapeDefaults",
    "doNotEmbedSmartTags",
    "decimalSymbol",
    "listSeparator",
];

/// Hyphenation settings in CT_Settings order: §17.15.1.10 autoHyphenation,
/// §17.15.1.21 consecutiveHyphenLimit, §17.15.1.52 hyphenationZone, §17.15.1.40
/// doNotHyphenateCaps (defaults omitted).
fn hyphenation_xml(h: &bo_model::Hyphenation) -> String {
    let mut s = String::new();
    if h.auto {
        s.push_str("<w:autoHyphenation/>");
    }
    if let Some(n) = h.consecutive_limit.filter(|n| *n > 0) {
        let _ = write!(s, "<w:consecutiveHyphenLimit w:val=\"{n}\"/>");
    }
    if let Some(z) = h.zone {
        let _ = write!(s, "<w:hyphenationZone w:val=\"{}\"/>", z.0.max(0));
    }
    if h.no_caps {
        s.push_str("<w:doNotHyphenateCaps/>");
    }
    s
}

fn remove_empty_element(xml: &str, tag: &str) -> String {
    let mut s = xml.to_owned();
    while let Some(at) = find_element(&s, tag) {
        let Some(gt) = s[at..].find('>') else { break };
        let mut end = at + gt + 1;
        if !s[..end].ends_with("/>") {
            let close = format!("</{tag}>");
            match s[end..].find(&close) {
                Some(c) => end += c + close.len(),
                None => break,
            }
        }
        s.replace_range(at..end, "");
    }
    s
}

impl Writer<'_> {
    fn write_main(&mut self, main: &str) -> Result<()> {
        let (prefix, suffix) = match &self.doc.source {
            Some(s) if !s.body_prefix.is_empty() => (s.body_prefix.clone(), s.body_suffix.clone()),
            _ => (
                format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:document xmlns:w=\"{W_NS}\" xmlns:r=\"{R_NS}\" xmlns:wp=\"{WP_NS}\" xmlns:a=\"{A_NS}\" xmlns:pic=\"{PIC_NS}\"><w:body>"
                ),
                "</w:body></w:document>".to_owned(),
            ),
        };
        let mut body = String::new();
        let mut cc_depth = 0;
        let n = self.doc.sections.len();
        for (si, sec) in self.doc.sections.iter().enumerate() {
            let last = si + 1 == n;
            let sect_unchanged = sec
                .src
                .as_ref()
                .is_some_and(|r| r.fingerprint == sec.props_fingerprint());
            let count = sec.blocks.len();
            for (bi, b) in sec.blocks.iter().enumerate() {
                let carries_sect = !last && bi + 1 == count;
                // §17.5.2.29 block-level sdt around paragraphs.
                open_block_controls(&mut body, b, &mut cc_depth);
                match b {
                    Block::Paragraph(p) if carries_sect => {
                        // The paragraph mark holds this section's sectPr (§17.6.17).
                        let para_unchanged = p
                            .src
                            .as_ref()
                            .is_some_and(|r| r.fingerprint == p.content_fingerprint());
                        if para_unchanged && sect_unchanged {
                            body.push_str(
                                &p.src
                                    .as_ref()
                                    .map(|r| r.xml.to_string())
                                    .unwrap_or_default(),
                            );
                        } else {
                            let sp = self.sect_pr(sec)?;
                            body.push_str(&self.paragraph(p, Some(&sp)));
                        }
                    }
                    _ => {
                        let xml = self.block(b);
                        body.push_str(&xml);
                        if carries_sect {
                            let sp = self.sect_pr(sec)?;
                            body.push_str(&format!("<w:p><w:pPr>{sp}</w:pPr></w:p>"));
                        }
                    }
                }
                close_block_controls(&mut body, b, &mut cc_depth);
            }
            close_all_block_controls(&mut body, &mut cc_depth);
            if last {
                let sp = self.sect_pr(sec)?;
                body.push_str(&sp);
            }
        }
        let bg = self.doc.design.background;
        let prefix = match &self.doc.source {
            Some(s) if s.background == bg => prefix,
            _ => with_background(&prefix, bg),
        };
        let xml = format!("{prefix}{body}{suffix}");
        if self.doc.source.is_some() {
            self.pkg.put_part(main, xml.into_bytes());
        } else {
            self.pkg.add_part(main, CT_DOCUMENT, xml.into_bytes())?;
        }
        Ok(())
    }

    fn block(&mut self, b: &Block) -> String {
        match b {
            Block::Paragraph(p) => match &p.src {
                Some(r) if r.fingerprint == p.content_fingerprint() => r.xml.to_string(),
                _ => self.paragraph(p, None),
            },
            Block::Table(t) => match &t.src {
                Some(r) if r.fingerprint == t.content_fingerprint() => r.xml.to_string(),
                _ => self.table(t),
            },
            Block::Opaque(r) => r.xml.to_string(),
        }
    }

    fn blocks(&mut self, blocks: &[Block]) -> String {
        let mut s = String::new();
        let mut depth = 0;
        for b in blocks {
            open_block_controls(&mut s, b, &mut depth);
            s.push_str(&self.block(b));
            close_block_controls(&mut s, b, &mut depth);
        }
        close_all_block_controls(&mut s, &mut depth);
        s
    }

    // ------------------------------------------------------------ paragraph

    fn paragraph(&mut self, p: &Paragraph, sect_pr: Option<&str>) -> String {
        let outer_sdt = std::mem::take(&mut self.sdt_depth);
        let mut s = String::from("<w:p>");
        let mut props = ppr_children(&p.props);
        if let Some(sp) = sect_pr {
            props.push(("sectPr".into(), sp.to_owned()));
        }
        let ppr = ordered(PPR_ORDER, props);
        if !ppr.is_empty() {
            let _ = write!(s, "<w:pPr>{ppr}</w:pPr>");
        }
        let mut i = 0;
        while i < p.runs.len() {
            let run = &p.runs[i];
            if let Some(link) = &run.props.link {
                // Group consecutive runs with the same link into one w:hyperlink (§17.16.22).
                let mut j = i;
                while j < p.runs.len() && p.runs[j].props.link.as_ref() == Some(link) {
                    j += 1;
                }
                let inner = self.tracked_runs(&p.runs[i..j]);
                if let Some(anchor) = link.strip_prefix('#') {
                    let _ = write!(
                        s,
                        "<w:hyperlink w:anchor=\"{}\">{inner}</w:hyperlink>",
                        escape(anchor)
                    );
                } else {
                    let id = self.rels.id_for(REL_HYPERLINK, link, true);
                    let _ = write!(
                        s,
                        "<w:hyperlink xmlns:r=\"{R_NS}\" r:id=\"{id}\">{inner}</w:hyperlink>"
                    );
                }
                i = j;
            } else {
                let mut j = i;
                while j < p.runs.len() && p.runs[j].props.link.is_none() {
                    j += 1;
                }
                let inner = self.tracked_runs(&p.runs[i..j]);
                s.push_str(&inner);
                i = j;
            }
        }
        // A start whose end was split into another paragraph closes here.
        for _ in 0..self.sdt_depth {
            s.push_str("</w:sdtContent></w:sdt>");
        }
        self.sdt_depth = outer_sdt;
        s.push_str("</w:p>");
        s
    }

    /// Runs wrapped in their tracked-change containers: consecutive runs with the same
    /// insertion share one `w:ins`/`w:moveTo` (§17.13.5.18 / §17.13.5.25), deletions a
    /// `w:del`/`w:moveFrom` inside it (§17.13.5.14 / §17.13.5.22).
    fn tracked_runs(&mut self, runs: &[Run]) -> String {
        let mut s = String::new();
        let mut i = 0;
        while i < runs.len() {
            let ins = runs[i].props.ins.clone();
            let mut j = i;
            while j < runs.len() && runs[j].props.ins == ins {
                j += 1;
            }
            let mut inner = String::new();
            let mut k = i;
            while k < j {
                let del = runs[k].props.del.clone();
                let mut m = k;
                let mut x = String::new();
                while m < j && runs[m].props.del == del {
                    x.push_str(&self.run(&runs[m]));
                    m += 1;
                }
                match &del {
                    Some(d) => inner.push_str(&rev_wrap(
                        if d.moved { "moveFrom" } else { "del" },
                        &d.info,
                        &x,
                    )),
                    None => inner.push_str(&x),
                }
                k = m;
            }
            match &ins {
                Some(m) => s.push_str(&rev_wrap(
                    if m.moved { "moveTo" } else { "ins" },
                    &m.info,
                    &inner,
                )),
                None => s.push_str(&inner),
            }
            i = j;
        }
        s
    }

    fn run(&mut self, run: &Run) -> String {
        let rpr = ordered(RPR_ORDER, rpr_children(&run.props));
        let rpr = if rpr.is_empty() {
            String::new()
        } else {
            format!("<w:rPr>{rpr}</w:rPr>")
        };
        // §17.3.3.7 delText / §17.16.13 delInstrText inside w:del.
        let deleted = run.props.del.as_ref().is_some_and(|d| !d.moved);
        let text_tag = if deleted { "delText" } else { "t" };
        // fldSimple may not appear inside a run-level track change (CT_RunTrackChange):
        // tracked fields are written as complex fields (§17.16.18 fldChar).
        let complex_fields = run.props.ins.is_some() || run.props.del.is_some();
        let mut out = String::new();
        let mut cur = String::new();
        let flush = |out: &mut String, cur: &mut String| {
            if !cur.is_empty() {
                let _ = write!(out, "<w:r>{rpr}{cur}</w:r>");
                cur.clear();
            }
        };
        for c in &run.content {
            match c {
                RunContent::Text(t) => {
                    let _ = write!(
                        cur,
                        "<w:{text_tag} xml:space=\"preserve\">{}</w:{text_tag}>",
                        escape(t)
                    );
                }
                RunContent::Tab => cur.push_str("<w:tab/>"),
                RunContent::LineBreak => cur.push_str("<w:br/>"),
                RunContent::PageBreak => cur.push_str("<w:br w:type=\"page\"/>"),
                RunContent::ColumnBreak => cur.push_str("<w:br w:type=\"column\"/>"),
                RunContent::NoBreakHyphen => cur.push_str("<w:noBreakHyphen/>"),
                RunContent::SoftHyphen => cur.push_str("<w:softHyphen/>"),
                RunContent::FootnoteRef(id) => {
                    let _ = write!(cur, "<w:footnoteReference w:id=\"{id}\"/>");
                }
                RunContent::EndnoteRef(id) => {
                    let _ = write!(cur, "<w:endnoteReference w:id=\"{id}\"/>");
                }
                RunContent::NoteMark => cur.push_str(if self.in_endnote {
                    "<w:endnoteRef/>"
                } else {
                    "<w:footnoteRef/>"
                }),
                RunContent::Opaque(r) => cur.push_str(&r.xml),
                RunContent::Image(img) => {
                    let xml = match &img.src {
                        // SmartArt / fallback pictures (media is a drawing part) stay verbatim.
                        Some(r)
                            if !r.xml.is_empty()
                                && img.shape.is_none()
                                && (!Self::image_changed(img) || img.media.ends_with(".xml")) =>
                        {
                            r.xml.to_string()
                        }
                        _ => self.drawing(img),
                    };
                    cur.push_str(&xml);
                }
                RunContent::Field {
                    instr,
                    cached,
                    locked,
                } if complex_fields => {
                    flush(&mut out, &mut cur);
                    let it = if deleted { "delInstrText" } else { "instrText" };
                    // §17.16.18 fldLock on the field's begin character.
                    let lock = if *locked { " w:fldLock=\"1\"" } else { "" };
                    let _ = write!(
                        out,
                        "<w:r>{rpr}<w:fldChar w:fldCharType=\"begin\"{lock}/></w:r><w:r>{rpr}<w:{it} xml:space=\"preserve\"> {} </w:{it}></w:r><w:r>{rpr}<w:fldChar w:fldCharType=\"separate\"/></w:r><w:r>{rpr}<w:{text_tag} xml:space=\"preserve\">{}</w:{text_tag}></w:r><w:r>{rpr}<w:fldChar w:fldCharType=\"end\"/></w:r>",
                        escape(instr),
                        escape(cached)
                    );
                }
                RunContent::Field {
                    instr,
                    cached,
                    locked,
                } => {
                    flush(&mut out, &mut cur);
                    // §17.16.19 fldSimple with cached result.
                    let _ = write!(
                        out,
                        "<w:fldSimple w:instr=\"{}\"{}><w:r>{rpr}<w:t xml:space=\"preserve\">{}</w:t></w:r></w:fldSimple>",
                        escape(&format!(" {instr} ")),
                        // §17.16.18 fldLock.
                        if *locked { " w:fldLock=\"1\"" } else { "" },
                        escape(cached)
                    );
                }
                RunContent::Markup(r) => {
                    flush(&mut out, &mut cur);
                    out.push_str(&r.xml);
                }
                // §17.13.4.4 / §17.13.4.3: range marks live outside runs.
                RunContent::CommentStart(id) => {
                    flush(&mut out, &mut cur);
                    let _ = write!(out, "<w:commentRangeStart w:id=\"{id}\"/>");
                }
                RunContent::CommentEnd(id) => {
                    flush(&mut out, &mut cur);
                    let _ = write!(out, "<w:commentRangeEnd w:id=\"{id}\"/>");
                }
                // §17.13.4.5 commentReference
                RunContent::CommentRef(id) => {
                    let _ = write!(cur, "<w:commentReference w:id=\"{id}\"/>");
                }
                // §17.5.2.31 sdt around runs; block-level controls are written around the
                // paragraphs (`blocks`).
                RunContent::ControlStart(cc) => {
                    flush(&mut out, &mut cur);
                    if !cc.block {
                        let _ = write!(out, "<w:sdt>{}<w:sdtContent>", sdt_pr(cc));
                        self.sdt_depth += 1;
                    }
                }
                RunContent::ControlEnd(block) => {
                    flush(&mut out, &mut cur);
                    // An end whose start was split into another paragraph is dropped.
                    if !block && self.sdt_depth > 0 {
                        out.push_str("</w:sdtContent></w:sdt>");
                        self.sdt_depth -= 1;
                    }
                }
            }
        }
        flush(&mut out, &mut cur);
        if run.content.is_empty() {
            let _ = write!(out, "<w:r>{rpr}</w:r>");
        }
        out
    }

    /// Source drawing XML is reused only when the picture's size/crop/rotation are unchanged.
    fn image_changed(img: &Image) -> bool {
        let Some(src) = &img.src else { return true };
        let x = &src.xml;
        // A moved floating picture (§20.4.2.3 anchor position) is written again; the
        // reader stores the fingerprint of the position it read.
        let moved = src.fingerprint != 0 && src.fingerprint != fingerprint(&img.anchor);
        moved
            || !(x.contains(&format!("cx=\"{}\"", img.width.0))
                && x.contains(&format!("cy=\"{}\"", img.height.0)))
    }

    /// Generate a `w:drawing` for a picture (§20.4.2.8 inline / §20.4.2.3 anchor).
    fn drawing(&mut self, img: &Image) -> String {
        if let Some(s) = &img.shape {
            // Shapes / text boxes: unchanged ⇒ source XML; else a new `wps:wsp`.
            if let Some(r) = img.src.as_ref().filter(|r| {
                !r.xml.is_empty() && r.fingerprint == crate::shape::shape_fingerprint(img)
            }) {
                return r.xml.to_string();
            }
            if let Some(art) = &s.art {
                return self.vml_art(img, s, art);
            }
            let txbx = s.text.as_ref().map(|b| {
                if b.is_empty() {
                    "<w:p/>".to_owned()
                } else {
                    self.blocks(b)
                }
            });
            self.drawing_id += 1;
            let id = self.drawing_id;
            let graphic = crate::shape::shape_graphic(img, s, id, txbx.as_deref());
            return Self::frame(img, id, "Shape", &graphic);
        }
        self.drawing_id += 1;
        let id = self.drawing_id;
        let rid = self.rels.id_for(REL_IMAGE, &img.media, false);
        let (cx, cy) = (img.width.0, img.height.0);
        let crop = img.crop.map_or(String::new(), |c| {
            format!(
                "<a:srcRect l=\"{}\" t=\"{}\" r=\"{}\" b=\"{}\"/>",
                c[0], c[1], c[2], c[3]
            )
        });
        let rot = if img.rotation == 0 {
            String::new()
        } else {
            format!(" rot=\"{}\"", img.rotation)
        };
        let graphic = format!(
            "<a:graphic xmlns:a=\"{A_NS}\"><a:graphicData uri=\"{PIC_NS}\"><pic:pic xmlns:pic=\"{PIC_NS}\"><pic:nvPicPr><pic:cNvPr id=\"{id}\" name=\"Picture {id}\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip xmlns:r=\"{R_NS}\" r:embed=\"{rid}\"/>{crop}<a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm{rot}><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic>"
        );
        Self::frame(img, id, "Picture", &graphic)
    }

    /// A text-path or picture shape (watermarks) as VML in `w:pict` (ECMA-376 Part 1,
    /// §17.3.3.19 pict; Part 4, §19.1.2.19 shape, §19.1.2.23 textpath, §19.1.2.11
    /// imagedata, §19.1.2.5 fill, §19.1.2.14 path). Namespaces are declared on the
    /// `w:pict` element so the part's root need not know VML.
    fn vml_art(&mut self, img: &Image, s: &bo_model::Shape, art: &bo_model::ShapeArt) -> String {
        const V_NS: &str = "urn:schemas-microsoft-com:vml";
        const O_NS: &str = "urn:schemas-microsoft-com:office:office";
        self.drawing_id += 1;
        let id = self.drawing_id;
        let pt = |e: bo_model::Emu| {
            let v = (f64::from(e.to_pt()) * 100.0).round() / 100.0;
            format!("{v}pt")
        };
        let mut style = String::from("position:absolute");
        let a = img.anchor.clone().unwrap_or_default();
        let _ = write!(
            style,
            ";margin-left:{};margin-top:{};width:{};height:{}",
            pt(a.x),
            pt(a.y),
            pt(img.width),
            pt(img.height)
        );
        if img.rotation != 0 {
            let deg = f64::from(img.rotation) / 60_000.0;
            let _ = write!(style, ";rotation:{deg}");
        }
        // z-index below 0 puts the shape behind the text.
        let z = if a.behind {
            -251_650_000 + i64::from(id)
        } else {
            i64::from(id)
        };
        let _ = write!(style, ";z-index:{z}");
        // Office VML extension: alignment and reference of the position.
        let rel = |r: &str, text: &'static str| match r {
            "page" => "page",
            "margin" => "margin",
            _ => text,
        };
        if let Some(al) = &a.h_align {
            let _ = write!(style, ";mso-position-horizontal:{}", escape(al));
        }
        let _ = write!(
            style,
            ";mso-position-horizontal-relative:{}",
            rel(&a.h_relative, "text")
        );
        if let Some(al) = &a.v_align {
            let _ = write!(style, ";mso-position-vertical:{}", escape(al));
        }
        let _ = write!(
            style,
            ";mso-position-vertical-relative:{}",
            rel(&a.v_relative, "text")
        );
        let name = if art.watermark {
            format!("BlueOfficeWatermark{id}")
        } else {
            format!("BlueOfficeArt{id}")
        };
        let mut x = format!("<w:pict xmlns:v=\"{V_NS}\" xmlns:o=\"{O_NS}\">");
        let opacity = if art.opacity < 1000 {
            format!("<v:fill opacity=\"{}\"/>", f64::from(art.opacity) / 1000.0)
        } else {
            String::new()
        };
        if let Some(t) = &art.text {
            let fill = s.fill.unwrap_or(bo_model::Color {
                r: 192,
                g: 192,
                b: 192,
            });
            let mut font = format!("font-family:&quot;{}&quot;;font-size:1pt", escape(&t.font));
            if t.bold {
                font.push_str(";font-weight:bold");
            }
            if t.italic {
                font.push_str(";font-style:italic");
            }
            let _ = write!(
                x,
                "<v:shape id=\"{name}\" o:spt=\"136\" style=\"{style}\" o:allowincell=\"f\" fillcolor=\"#{}\" stroked=\"f\" coordsize=\"21600,21600\" path=\"m0,10800l21600,10800e\">{opacity}<v:path textpathok=\"t\"/><v:textpath on=\"t\" fitshape=\"t\" style=\"{font}\" string=\"{}\"/></v:shape>",
                fill.to_hex(),
                escape(&t.text)
            );
        } else if let Some(p) = &art.picture {
            let rid = self.rels.id_for(REL_IMAGE, p, false);
            // §19.1.2.11 imagedata gain / blacklevel: lower contrast, lighter picture.
            let wash = if art.washout {
                " gain=\"0.3\" blacklevel=\"0.35\""
            } else {
                ""
            };
            let _ = write!(
                x,
                "<v:shape id=\"{name}\" o:spt=\"75\" style=\"{style}\" o:allowincell=\"f\" stroked=\"f\" filled=\"f\" coordsize=\"21600,21600\" path=\"m0,0l21600,0,21600,21600,0,21600xe\"><v:imagedata xmlns:r=\"{R_NS}\" r:id=\"{rid}\" o:title=\"\"{wash}/></v:shape>"
            );
        }
        x.push_str("</w:pict>");
        x
    }

    /// `w:drawing` with `graphic` placed inline (§20.4.2.8) or anchored (§20.4.2.3).
    fn frame(img: &Image, id: u32, kind: &str, graphic: &str) -> String {
        let (cx, cy) = (img.width.0, img.height.0);
        let descr = escape(&img.description);
        match &img.anchor {
            None => format!(
                "<w:drawing><wp:inline xmlns:wp=\"{WP_NS}\" distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:docPr id=\"{id}\" name=\"{kind} {id}\" descr=\"{descr}\"/>{graphic}</wp:inline></w:drawing>"
            ),
            Some(a) => {
                // §20.4.3.7 ST_WrapText
                let side = match a.wrap_text {
                    WrapText::BothSides => "bothSides",
                    WrapText::Left => "left",
                    WrapText::Right => "right",
                    WrapText::Largest => "largest",
                };
                let [dt, db, dl, dr] = a.dist.map(|d| d.0);
                let wrap = match a.wrap {
                    Wrap::None => "<wp:wrapNone/>".to_owned(),
                    // §20.4.2.17 wrapSquare
                    Wrap::Square => format!("<wp:wrapSquare wrapText=\"{side}\"/>"),
                    // §20.4.2.20 wrapTight with its §20.4.2.16 wrapPolygon: the
                    // points read from the file (§20.4.2.14 start, §20.4.2.9
                    // lineTo), else the rectangle of the bounding box.
                    Wrap::Tight => {
                        let pts: Vec<(i64, i64)> = if a.polygon_points.len() >= 3 {
                            a.polygon_points.clone()
                        } else {
                            let [x0, y0, x1, y1] = a.polygon.unwrap_or([0, 0, 21_600, 21_600]);
                            vec![(x0, y0), (x0, y1), (x1, y1), (x1, y0), (x0, y0)]
                        };
                        let mut poly = String::new();
                        for (i, (x, y)) in pts.iter().enumerate() {
                            let tag = if i == 0 { "start" } else { "lineTo" };
                            poly.push_str(&format!("<wp:{tag} x=\"{x}\" y=\"{y}\"/>"));
                        }
                        let edited = u8::from(a.polygon_points.len() >= 3);
                        format!("<wp:wrapTight wrapText=\"{side}\"><wp:wrapPolygon edited=\"{edited}\">{poly}</wp:wrapPolygon></wp:wrapTight>")
                    }
                    Wrap::TopAndBottom => "<wp:wrapTopAndBottom/>".to_owned(),
                };
                // §20.4.2.1 / §20.4.2.2 align, else §20.4.2.12 posOffset.
                let pos = |align: &Option<String>, off: i64| match align {
                    Some(al) => format!("<wp:align>{}</wp:align>", escape(al)),
                    None => format!("<wp:posOffset>{off}</wp:posOffset>"),
                };
                format!(
                    "<w:drawing><wp:anchor xmlns:wp=\"{WP_NS}\" distT=\"{dt}\" distB=\"{db}\" distL=\"{dl}\" distR=\"{dr}\" simplePos=\"0\" relativeHeight=\"{id}\" behindDoc=\"{}\" locked=\"0\" layoutInCell=\"1\" allowOverlap=\"1\"><wp:simplePos x=\"0\" y=\"0\"/><wp:positionH relativeFrom=\"{}\">{}</wp:positionH><wp:positionV relativeFrom=\"{}\">{}</wp:positionV><wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/>{wrap}<wp:docPr id=\"{id}\" name=\"{kind} {id}\" descr=\"{descr}\"/><wp:cNvGraphicFramePr/>{graphic}</wp:anchor></w:drawing>",
                    u8::from(a.behind),
                    escape(&a.h_relative),
                    pos(&a.h_align, a.x.0),
                    escape(&a.v_relative),
                    pos(&a.v_align, a.y.0)
                )
            }
        }
    }

    // ------------------------------------------------------------ table

    fn table(&mut self, t: &Table) -> String {
        let mut s = String::from("<w:tbl>");
        let tblpr = ordered(TBLPR_ORDER, tblpr_children(&t.props));
        let _ = write!(s, "<w:tblPr>{tblpr}</w:tblPr><w:tblGrid>");
        let cols = |s: &mut String, grid: &[bo_model::Twips]| {
            for g in grid {
                let _ = write!(s, "<w:gridCol w:w=\"{}\"/>", g.0);
            }
        };
        cols(&mut s, &t.grid);
        // §17.13.5.33 tblGridChange (CT_Markup: only w:id) with the previous grid.
        if let Some(gc) = &t.grid_change {
            let _ = write!(
                s,
                "<w:tblGridChange w:id=\"{}\"><w:tblGrid>",
                unique_id(gc.info.id)
            );
            cols(&mut s, &gc.old);
            s.push_str("</w:tblGrid></w:tblGridChange>");
        }
        s.push_str("</w:tblGrid>");
        for row in &t.rows {
            s.push_str("<w:tr>");
            // CT_TrPrBase children (§17.4.82 trPr).
            let base =
                |header: bool, cant_split: bool, height: Option<(bo_model::Twips, HeightRule)>| {
                    let mut v: Vec<String> = Vec::new();
                    if cant_split {
                        v.push("<w:cantSplit/>".into());
                    }
                    if let Some((h, rule)) = height {
                        v.push(format!(
                            "<w:trHeight w:val=\"{}\"{}/>",
                            h.0,
                            if rule == HeightRule::Exact {
                                " w:hRule=\"exact\""
                            } else {
                                ""
                            }
                        ));
                    }
                    if header {
                        v.push("<w:tblHeader/>".into());
                    }
                    v
                };
            let mut trpr = base(row.props.header, row.props.cant_split, row.props.height);
            trpr.extend(
                row.props
                    .unknown
                    .iter()
                    .filter(|r| raw_local_name(&r.xml) != "trPrChange")
                    .map(|r| r.xml.to_string()),
            );
            // CT_TrPr: §17.13.5.19 ins, §17.13.5.12 del, then §17.13.5.37 trPrChange.
            if let Some(m) = &row.props.ins {
                trpr.push(format!("<w:ins{}/>", rev_attrs(&m.info)));
            }
            if let Some(m) = &row.props.del {
                trpr.push(format!("<w:del{}/>", rev_attrs(&m.info)));
            }
            if let Some(ch) = &row.props.change {
                trpr.push(format!(
                    "<w:trPrChange{}><w:trPr>{}</w:trPr></w:trPrChange>",
                    rev_attrs(&ch.info),
                    base(ch.header, ch.cant_split, ch.height).concat()
                ));
            }
            if !trpr.is_empty() {
                let _ = write!(s, "<w:trPr>{}</w:trPr>", trpr.concat());
            }
            for c in &row.cells {
                s.push_str(&self.cell(c));
            }
            s.push_str("</w:tr>");
        }
        s.push_str("</w:tbl>");
        s
    }

    fn cell(&mut self, c: &Cell) -> String {
        let items = tcpr_items(&c.props);
        let tcpr = ordered(TCPR_ORDER, items);
        let mut content = self.blocks(&c.blocks);
        // §17.4.66: a cell must end with a paragraph.
        if !matches!(
            c.blocks.last(),
            Some(Block::Paragraph(_) | Block::Opaque(_))
        ) {
            content.push_str("<w:p/>");
        }
        format!("<w:tc><w:tcPr>{tcpr}</w:tcPr>{content}</w:tc>")
    }

    // ------------------------------------------------------------ sections

    fn sect_pr(&mut self, sec: &Section) -> Result<String> {
        if let Some(r) = &sec.src {
            if r.fingerprint == sec.props_fingerprint() {
                return Ok(r.xml.to_string());
            }
        }
        let mut s = String::from("<w:sectPr>");
        for (is_header, kind, blocks) in crate::reader::header_footer_sets(sec) {
            self.hf_counter += 1;
            let tag = if is_header { "hdr" } else { "ftr" };
            let file = format!(
                "{}bo{}.xml",
                if is_header { "header" } else { "footer" },
                self.hf_counter
            );
            let part = format!("/word/{file}");
            // The header part gets relationships of its own (pictures, links), starting
            // from those of the part it was read from so that verbatim content keeps
            // resolving its `r:id`s (ECMA-376 Part 2, §9.3 relationships are per part).
            let src_part = sec.hf_parts.part(is_header, kind);
            let inherited = src_part
                .and_then(|src| {
                    let rels = self.pkg.relationships(src).ok()?;
                    Some(
                        rels.into_iter()
                            .map(|mut r| {
                                // Internal targets made absolute (valid from any part).
                                if r.mode == TargetMode::Internal {
                                    r.target = bo_opc::resolve_target(src, &r.target);
                                }
                                r
                            })
                            .collect::<Vec<_>>(),
                    )
                })
                .unwrap_or_default();
            let main_rels = std::mem::replace(
                &mut self.rels,
                Rels {
                    part: part.clone(),
                    list: Vec::new(),
                    changed: false,
                },
            );
            self.rels.list = inherited;
            let body = self.blocks(blocks);
            let part_rels = std::mem::replace(&mut self.rels, main_rels);
            let body = if body.is_empty() {
                "<w:p/>".to_owned()
            } else {
                body
            };
            // Root start tag: the source part's (its namespace declarations serve the
            // verbatim content), completed with the ones generated content uses.
            let mut root = src_part
                .and_then(|src| self.pkg.part(src))
                .and_then(|p| {
                    let x = String::from_utf8_lossy(&p.data).into_owned();
                    let at = find_element(&x, &format!("w:{tag}"))?;
                    let end = at + x[at..].find('>')?;
                    Some(x[at..end].trim_end_matches('/').to_owned() + ">")
                })
                .unwrap_or_else(|| format!("<w:{tag} xmlns:w=\"{W_NS}\">"));
            for (prefix, ns) in [("r", R_NS), ("wp", WP_NS), ("a", A_NS), ("pic", PIC_NS)] {
                root = with_namespace(&root, prefix, ns);
            }
            let xml = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n{root}{body}</w:{tag}>"
            );
            self.pkg.add_part(
                &part,
                if is_header { CT_HEADER } else { CT_FOOTER },
                xml.into_bytes(),
            )?;
            if !part_rels.list.is_empty() {
                self.pkg.set_relationships(&part, &part_rels.list);
            }
            let id = self.rels.id_for(
                if is_header { REL_HEADER } else { REL_FOOTER },
                &part,
                false,
            );
            let el = if is_header {
                "headerReference"
            } else {
                "footerReference"
            };
            let _ = write!(
                s,
                "<w:{el} xmlns:r=\"{R_NS}\" w:type=\"{kind}\" r:id=\"{id}\"/>"
            );
        }
        s.push_str(&sect_props_xml(sec));
        // §17.13.5.32 sectPrChange: the previous section properties (no references).
        if let Some(ch) = &sec.change {
            let old = Section {
                page: ch.page,
                columns: ch.columns.clone(),
                title_page: ch.title_page,
                break_type: ch.break_type,
                page_number_start: ch.page_number_start,
                footnote_pr: sec.footnote_pr.clone(),
                endnote_pr: sec.endnote_pr.clone(),
                ..Section::default()
            };
            let _ = write!(
                s,
                "<w:sectPrChange{}><w:sectPr>{}</w:sectPr></w:sectPrChange>",
                rev_attrs(&ch.info),
                sect_props_xml(&old)
            );
        }
        s.push_str("</w:sectPr>");
        Ok(s)
    }

    // ------------------------------------------------------------ parts

    fn write_styles(&mut self, main: &str) -> Result<()> {
        let doc = self.doc;
        if let Some(src) = &doc.source {
            if src.styles_fingerprint == crate::reader::styles_fingerprint(doc) {
                return Ok(());
            }
        }
        let mut s = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:styles xmlns:w=\"{W_NS}\"><w:docDefaults><w:rPrDefault><w:rPr>{}</w:rPr></w:rPrDefault><w:pPrDefault><w:pPr>{}</w:pPr></w:pPrDefault></w:docDefaults>",
            ordered(RPR_ORDER, rpr_children(&doc.default_run)),
            ordered(PPR_ORDER, ppr_children(&doc.default_para))
        );
        if let Some(l) = &doc.styles.latent {
            s.push_str(&l.xml);
        }
        for st in &doc.styles.styles {
            let kind = match st.kind {
                StyleKind::Paragraph => "paragraph",
                StyleKind::Character => "character",
                StyleKind::Table => "table",
                StyleKind::Numbering => "numbering",
            };
            let _ = write!(
                s,
                "<w:style w:type=\"{kind}\"{} w:styleId=\"{}\"><w:name w:val=\"{}\"/>",
                if st.is_default {
                    " w:default=\"1\""
                } else {
                    ""
                },
                escape(&st.id),
                escape(&st.name)
            );
            if let Some(b) = &st.based_on {
                let _ = write!(s, "<w:basedOn w:val=\"{}\"/>", escape(b));
            }
            if let Some(n) = &st.next {
                let _ = write!(s, "<w:next w:val=\"{}\"/>", escape(n));
            }
            if let Some(l) = &st.linked {
                let _ = write!(s, "<w:link w:val=\"{}\"/>", escape(l));
            }
            if st.quick {
                s.push_str("<w:qFormat/>");
            }
            let ppr = ordered(PPR_ORDER, ppr_children(&st.ppr));
            if !ppr.is_empty() {
                let _ = write!(s, "<w:pPr>{ppr}</w:pPr>");
            }
            let rpr = ordered(RPR_ORDER, rpr_children(&st.rpr));
            if !rpr.is_empty() {
                let _ = write!(s, "<w:rPr>{rpr}</w:rPr>");
            }
            if let Some(t) = &st.tblpr {
                let _ = write!(
                    s,
                    "<w:tblPr>{}</w:tblPr>",
                    ordered(TBLPR_ORDER, tblpr_children(t))
                );
            }
            s.push_str("</w:style>");
        }
        s.push_str("</w:styles>");
        let existing = self.pkg.related_part(main, REL_STYLES)?;
        let part = existing.unwrap_or_else(|| "/word/styles.xml".into());
        self.pkg.add_part(&part, CT_STYLES, s.into_bytes())?;
        self.rels.id_for(REL_STYLES, &part, false);
        Ok(())
    }

    fn write_numbering(&mut self, main: &str) -> Result<()> {
        let doc = self.doc;
        if let Some(src) = &doc.source {
            if src.numbering_fingerprint == fingerprint(&doc.numbering) {
                return Ok(());
            }
        }
        if doc.numbering.abstracts.is_empty() && doc.numbering.nums.is_empty() {
            return Ok(());
        }
        let mut s = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:numbering xmlns:w=\"{W_NS}\">");
        for a in &doc.numbering.abstracts {
            let _ = write!(s, "<w:abstractNum w:abstractNumId=\"{}\">", a.id);
            for l in &a.levels {
                let _ = write!(
                    s,
                    "<w:lvl w:ilvl=\"{}\"><w:start w:val=\"{}\"/><w:numFmt w:val=\"{}\"/>",
                    l.ilvl,
                    l.start,
                    escape(l.format.as_str())
                );
                if let Some(rs) = l.restart {
                    let _ = write!(s, "<w:lvlRestart w:val=\"{rs}\"/>");
                }
                // §17.9.23 pStyle (CT_Lvl order: start, numFmt, lvlRestart, pStyle, isLgl, suff, lvlText, …).
                if let Some(ps) = &l.pstyle {
                    let _ = write!(s, "<w:pStyle w:val=\"{}\"/>", escape(ps));
                }
                if l.suffix != "tab" {
                    let _ = write!(s, "<w:suff w:val=\"{}\"/>", escape(&l.suffix));
                }
                let _ = write!(
                    s,
                    "<w:lvlText w:val=\"{}\"/><w:lvlJc w:val=\"{}\"/>",
                    escape(&l.text),
                    jc(l.align)
                );
                let ppr = ordered(PPR_ORDER, ppr_children(&l.ppr));
                if !ppr.is_empty() {
                    let _ = write!(s, "<w:pPr>{ppr}</w:pPr>");
                }
                let rpr = ordered(RPR_ORDER, rpr_children(&l.rpr));
                if !rpr.is_empty() {
                    let _ = write!(s, "<w:rPr>{rpr}</w:rPr>");
                }
                s.push_str("</w:lvl>");
            }
            s.push_str("</w:abstractNum>");
        }
        for n in &doc.numbering.nums {
            let _ = write!(
                s,
                "<w:num w:numId=\"{}\"><w:abstractNumId w:val=\"{}\"/>",
                n.id, n.abstract_id
            );
            for (l, st) in &n.start_overrides {
                let _ = write!(s, "<w:lvlOverride w:ilvl=\"{l}\"><w:startOverride w:val=\"{st}\"/></w:lvlOverride>");
            }
            s.push_str("</w:num>");
        }
        s.push_str("</w:numbering>");
        let part = self
            .pkg
            .related_part(main, REL_NUMBERING)?
            .unwrap_or_else(|| "/word/numbering.xml".into());
        self.pkg.add_part(&part, CT_NUMBERING, s.into_bytes())?;
        self.rels.id_for(REL_NUMBERING, &part, false);
        Ok(())
    }

    /// Footnote and endnote parts (§17.11.15 footnotes, §17.11.8 endnotes) of a new
    /// document, or of a read one whose notes changed (written to the existing parts).
    fn write_notes(&mut self, main: &str) -> Result<()> {
        let doc = self.doc;
        for (end, notes) in [(false, &doc.footnotes), (true, &doc.endnotes)] {
            if notes.is_empty() {
                continue;
            }
            let (root, item, ct, rel, default_part) = if end {
                (
                    "endnotes",
                    "endnote",
                    CT_ENDNOTES,
                    REL_ENDNOTES,
                    "/word/endnotes.xml",
                )
            } else {
                (
                    "footnotes",
                    "footnote",
                    CT_FOOTNOTES,
                    REL_FOOTNOTES,
                    "/word/footnotes.xml",
                )
            };
            let part = self
                .pkg
                .related_part(main, rel)?
                .unwrap_or_else(|| default_part.to_owned());
            let part = part.as_str();
            self.in_endnote = end;
            let mut s = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:{root} xmlns:w=\"{W_NS}\" xmlns:r=\"{R_NS}\" xmlns:wp=\"{WP_NS}\" xmlns:a=\"{A_NS}\" xmlns:pic=\"{PIC_NS}\">"
            );
            // §17.11.23 separator / §17.11.1 continuationSeparator.
            let _ = write!(
                s,
                "<w:{item} w:type=\"separator\" w:id=\"-1\"><w:p><w:r><w:separator/></w:r></w:p></w:{item}><w:{item} w:type=\"continuationSeparator\" w:id=\"0\"><w:p><w:r><w:continuationSeparator/></w:r></w:p></w:{item}>"
            );
            // §17.18.33 continuationNotice (footnotes only in the model): the first
            // id ≥ 1 no note uses (Word files give it 1 and number notes from 2).
            if let (false, Some(blocks)) = (end, &doc.continuation_notice) {
                let id = (1..i32::MAX).find(|i| !notes.contains_key(i)).unwrap_or(1);
                let body = self.blocks(blocks);
                let _ = write!(
                    s,
                    "<w:{item} w:type=\"continuationNotice\" w:id=\"{id}\">{body}</w:{item}>"
                );
            }
            for (id, blocks) in notes {
                let body = self.blocks(blocks);
                let body = if body.is_empty() {
                    "<w:p/>".to_owned()
                } else {
                    body
                };
                let _ = write!(s, "<w:{item} w:id=\"{id}\">{body}</w:{item}>");
            }
            let _ = write!(s, "</w:{root}>");
            self.pkg.add_part(part, ct, s.into_bytes())?;
            self.rels.id_for(rel, part, false);
        }
        self.in_endnote = false;
        Ok(())
    }

    /// Comments (ECMA-376 Part 1, §17.13.4.6 `comments.xml`) and their threading
    /// extension ([MS-DOCX] §2.5.2 `commentsExtended.xml`, `commentsIds.xml`).
    /// Parts of an unchanged source are kept verbatim.
    fn write_comments(&mut self, main: &str) -> Result<()> {
        let doc = self.doc;
        match &doc.source {
            Some(src) if src.comments_fingerprint == fingerprint(&doc.comments) => return Ok(()),
            None if doc.comments.is_empty() => return Ok(()),
            _ => {}
        }
        let root = doc
            .source
            .as_ref()
            .and_then(|s| s.comments_root.clone())
            .unwrap_or_else(|| {
                format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:comments xmlns:w=\"{W_NS}\" xmlns:r=\"{R_NS}\" xmlns:wp=\"{WP_NS}\" xmlns:a=\"{A_NS}\" xmlns:pic=\"{PIC_NS}\">")
            });
        let root = with_namespace(&root, "w14", W14_NS);
        let mut s = root;
        for c in &doc.comments {
            let _ = write!(
                s,
                "<w:comment w:id=\"{}\" w:author=\"{}\"",
                c.id,
                escape(&c.author)
            );
            if let Some(d) = &c.date {
                let _ = write!(s, " w:date=\"{}\"", escape(d));
            }
            if let Some(i) = &c.initials {
                let _ = write!(s, " w:initials=\"{}\"", escape(i));
            }
            s.push('>');
            let last = c
                .blocks
                .iter()
                .rposition(|b| matches!(b, Block::Paragraph(_)));
            let mut cc_depth = 0;
            for (i, b) in c.blocks.iter().enumerate() {
                open_block_controls(&mut s, b, &mut cc_depth);
                let xml = self.block(b);
                match (&c.para_id, Some(i) == last) {
                    (Some(pid), true) => s.push_str(&with_para_id(&xml, pid)),
                    _ => s.push_str(&xml),
                }
                close_block_controls(&mut s, b, &mut cc_depth);
            }
            close_all_block_controls(&mut s, &mut cc_depth);
            if last.is_none() {
                match &c.para_id {
                    Some(pid) => {
                        let _ = write!(s, "<w:p w14:paraId=\"{pid}\"/>");
                    }
                    None => s.push_str("<w:p/>"),
                }
            }
            s.push_str("</w:comment>");
        }
        s.push_str("</w:comments>");
        let part = self
            .pkg
            .related_part(main, crate::reader::REL_COMMENTS)?
            .unwrap_or_else(|| "/word/comments.xml".into());
        self.pkg.add_part(&part, CT_COMMENTS, s.into_bytes())?;
        self.rels.id_for(crate::reader::REL_COMMENTS, &part, false);

        // [MS-DOCX] §2.5.2 commentsEx: one entry per comment with a paraId.
        let mut ext = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w15:commentsEx xmlns:w15=\"{W15_NS}\" xmlns:mc=\"{MC_NS}\" mc:Ignorable=\"w15\">");
        let mut any = false;
        for c in &doc.comments {
            let Some(pid) = &c.para_id else { continue };
            any = true;
            let _ = write!(ext, "<w15:commentEx w15:paraId=\"{}\"", escape(pid));
            if let Some(parent) = &c.parent_para_id {
                let _ = write!(ext, " w15:paraIdParent=\"{}\"", escape(parent));
            }
            let _ = write!(ext, " w15:done=\"{}\"/>", u8::from(c.done));
        }
        ext.push_str("</w15:commentsEx>");
        let has_ext = self
            .pkg
            .related_part(main, crate::reader::REL_COMMENTS_EXT)?;
        if any || has_ext.is_some() {
            let part = has_ext.unwrap_or_else(|| "/word/commentsExtended.xml".into());
            self.pkg
                .add_part(&part, CT_COMMENTS_EXT, ext.into_bytes())?;
            self.rels
                .id_for(crate::reader::REL_COMMENTS_EXT, &part, false);
        }
        // commentsIds is written when the source had one or durable ids are known.
        let ids_part = self
            .pkg
            .related_part(main, crate::reader::REL_COMMENTS_IDS)?
            .or_else(|| {
                doc.comments
                    .iter()
                    .any(|c| c.durable_id.is_some())
                    .then(|| "/word/commentsIds.xml".to_owned())
            });
        if let Some(part) = ids_part {
            let mut ids = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w16cid:commentsIds xmlns:w16cid=\"{W16CID_NS}\" xmlns:mc=\"{MC_NS}\" mc:Ignorable=\"w16cid\">");
            for c in &doc.comments {
                let Some(pid) = &c.para_id else { continue };
                let durable = c
                    .durable_id
                    .clone()
                    .unwrap_or_else(|| format!("{:08X}", fingerprint(&(pid, c.id)) & 0x7FFF_FFFF));
                let _ = write!(
                    ids,
                    "<w16cid:commentId w16cid:paraId=\"{}\" w16cid:durableId=\"{}\"/>",
                    escape(pid),
                    escape(&durable)
                );
            }
            ids.push_str("</w16cid:commentsIds>");
            self.pkg
                .add_part(&part, CT_COMMENTS_IDS, ids.into_bytes())?;
            self.rels
                .id_for(crate::reader::REL_COMMENTS_IDS, &part, false);
        }
        Ok(())
    }

    /// Set or remove `w:evenAndOddHeaders` (§17.15.1.35) in the settings part of a read
    /// document, keeping everything else; the element goes before the first sibling
    /// that follows it in CT_Settings (§17.15.1.78).
    fn patch_settings(&mut self, main: &str) -> Result<()> {
        let Some(part) = self.pkg.related_part(main, REL_SETTINGS)? else {
            return self.write_settings(main);
        };
        let Some(xml) = self
            .pkg
            .part(&part)
            .map(|p| String::from_utf8_lossy(&p.data).into_owned())
        else {
            return self.write_settings(main);
        };
        let mut s = remove_empty_element(&xml, "w:evenAndOddHeaders");
        s = remove_empty_element(&s, "w:displayBackgroundShape");
        if self.doc.design.display_background_shape {
            // CT_Settings: displayBackgroundShape follows doNotDisplayPageBoundaries and
            // precedes printPostScriptOverText … defaultTabStop … (§17.15.1.78).
            const AFTER_BG: &[&str] = &[
                "printPostScriptOverText",
                "printFractionalCharacterWidth",
                "printFormsData",
                "embedTrueTypeFonts",
                "embedSystemFonts",
                "saveSubsetFonts",
                "saveFormsData",
                "mirrorMargins",
                "alignBordersAndEdges",
                "bordersDoNotSurroundHeader",
                "bordersDoNotSurroundFooter",
                "gutterAtTop",
                "hideSpellingErrors",
                "hideGrammaticalErrors",
                "activeWritingStyle",
                "proofState",
                "formsDesign",
                "attachedTemplate",
                "linkStyles",
                "stylePaneFormatFilter",
                "stylePaneSortMethod",
                "documentType",
                "mailMerge",
                "revisionView",
                "trackRevisions",
                "doNotTrackMoves",
                "doNotTrackFormatting",
                "documentProtection",
                "autoFormatOverride",
                "styleLockTheme",
                "styleLockQFSet",
                "defaultTabStop",
                "autoHyphenation",
                "consecutiveHyphenLimit",
                "hyphenationZone",
                "doNotHyphenateCaps",
                "showEnvelope",
                "summaryLength",
                "clickAndTypeStyle",
                "defaultTableStyle",
                "evenAndOddHeaders",
                "bookFoldRevPrinting",
                "bookFoldPrinting",
                "characterSpacingControl",
                "compat",
                "rsids",
                "themeFontLang",
                "clrSchemeMapping",
                "shapeDefaults",
                "decimalSymbol",
                "listSeparator",
            ];
            let at = AFTER_BG
                .iter()
                .filter_map(|n| find_element(&s, &format!("w:{n}")))
                .min()
                .or_else(|| s.rfind("</w:settings>"));
            if let Some(at) = at {
                s.insert_str(at, "<w:displayBackgroundShape/>");
            }
        }
        if self.doc.even_and_odd_headers {
            const AFTER: &[&str] = &[
                "bookFoldRevPrinting",
                "bookFoldPrinting",
                "bookFoldPrintingSheets",
                "drawingGridHorizontalSpacing",
                "drawingGridVerticalSpacing",
                "displayHorizontalDrawingGridEvery",
                "displayVerticalDrawingGridEvery",
                "doNotUseMarginsForDrawingGridOrigin",
                "drawingGridHorizontalOrigin",
                "drawingGridVerticalOrigin",
                "doNotShadeFormData",
                "noPunctuationKerning",
                "characterSpacingControl",
                "printTwoOnOne",
                "strictFirstAndLastChars",
                "noLineBreaksAfter",
                "noLineBreaksBefore",
                "savePreviewPicture",
                "doNotValidateAgainstSchema",
                "saveInvalidXml",
                "ignoreMixedContent",
                "alwaysShowPlaceholderText",
                "doNotDemarcateInvalidXml",
                "saveXmlDataOnly",
                "useXSLTWhenSaving",
                "saveThroughXslt",
                "showXMLTags",
                "alwaysMergeEmptyNamespace",
                "updateFields",
                "hdrShapeDefaults",
                "footnotePr",
                "endnotePr",
                "compat",
                "docVars",
                "rsids",
                "mathPr",
                "attachedSchema",
                "themeFontLang",
                "clrSchemeMapping",
                "doNotIncludeSubdocsInStats",
                "doNotAutoCompressPictures",
                "forceUpgrade",
                "captions",
                "readModeInkLockDown",
                "smartTagType",
                "schemaLibrary",
                "shapeDefaults",
                "doNotEmbedSmartTags",
                "decimalSymbol",
                "listSeparator",
            ];
            let at = AFTER
                .iter()
                .filter_map(|n| find_element(&s, &format!("w:{n}")))
                .min()
                .or_else(|| s.rfind("</w:settings>"));
            if let Some(at) = at {
                s.insert_str(at, "<w:evenAndOddHeaders/>");
            }
        }
        for tag in [
            "w:autoHyphenation",
            "w:consecutiveHyphenLimit",
            "w:hyphenationZone",
            "w:doNotHyphenateCaps",
        ] {
            s = remove_empty_element(&s, tag);
        }
        let hy = hyphenation_xml(&self.doc.hyphenation);
        if !hy.is_empty() {
            let at = AFTER_HYPHENATION
                .iter()
                .filter_map(|n| find_element(&s, &format!("w:{n}")))
                .min()
                .or_else(|| s.rfind("</w:settings>"));
            if let Some(at) = at {
                s.insert_str(at, &hy);
            }
        }
        self.pkg.put_part(&part, s.into_bytes());
        Ok(())
    }

    /// A theme authored by BlueOffice (ECMA-376 Part 1, §14.2.7 theme part; DrawingML
    /// §20.1.6.9 theme) replaces the source's theme part, or is added as
    /// `word/theme/theme1.xml`; without one the source part is kept.
    fn write_theme(&mut self, main: &str) -> Result<()> {
        const REL_THEME: &str =
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
        const CT_THEME: &str = "application/vnd.openxmlformats-officedocument.theme+xml";
        let Some(xml) = self.doc.design.theme_xml.clone() else {
            return Ok(());
        };
        let existing = if self.doc.source.is_some() {
            self.pkg.related_part(main, REL_THEME)?
        } else {
            None
        };
        if let Some(part) = existing {
            self.pkg.put_part(&part, xml.as_bytes().to_vec());
            return Ok(());
        }
        let mut part = "/word/theme/theme1.xml".to_owned();
        let mut n = 1;
        while self.pkg.part(&part).is_some() {
            n += 1;
            part = format!("/word/theme/theme{n}.xml");
        }
        self.pkg
            .add_part(&part, CT_THEME, xml.as_bytes().to_vec())?;
        self.rels.id_for(REL_THEME, &part, false);
        Ok(())
    }

    fn write_settings(&mut self, _main: &str) -> Result<()> {
        let mut s = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:settings xmlns:w=\"{W_NS}\">");
        // §17.15.1.32 displayBackgroundShape (before defaultTabStop).
        if self.doc.design.display_background_shape {
            s.push_str("<w:displayBackgroundShape/>");
        }
        // CT_Settings order (§17.15.1.78): defaultTabStop precedes evenAndOddHeaders.
        // §17.15.1.25 defaultTabStop (omitted ⇒ 720).
        if let Some(t) = self.doc.default_tab_stop {
            let _ = write!(s, "<w:defaultTabStop w:val=\"{}\"/>", t.0);
        }
        s.push_str(&hyphenation_xml(&self.doc.hyphenation));
        if self.doc.even_and_odd_headers {
            s.push_str("<w:evenAndOddHeaders/>");
        }
        // §17.11.11 footnotePr / §17.11.4 endnotePr (document-wide), before compat.
        s.push_str(&note_pr_xml("footnotePr", &self.doc.footnote_pr));
        s.push_str(&note_pr_xml("endnotePr", &self.doc.endnote_pr));
        let _ = write!(s, "<w:compat><w:compatSetting w:name=\"compatibilityMode\" w:uri=\"http://schemas.microsoft.com/office/word\" w:val=\"15\"/></w:compat></w:settings>");
        self.pkg
            .add_part("/word/settings.xml", CT_SETTINGS, s.into_bytes())?;
        self.rels.id_for(REL_SETTINGS, "/word/settings.xml", false);
        Ok(())
    }

    fn write_media(&mut self) -> Result<()> {
        let mut ct = self.pkg.content_types()?;
        for (name, data) in &self.doc.media {
            if self.pkg.part(name).is_none() {
                self.pkg.put_part(name, data.as_ref().clone());
            }
            let ext = name
                .rsplit_once('.')
                .map(|(_, e)| e.to_ascii_lowercase())
                .unwrap_or_default();
            let mime = match ext.as_str() {
                "png" => "image/png",
                "jpg" | "jpeg" => "image/jpeg",
                "gif" => "image/gif",
                "bmp" => "image/bmp",
                "tif" | "tiff" => "image/tiff",
                "emf" => "image/x-emf",
                "wmf" => "image/x-wmf",
                "svg" => "image/svg+xml",
                _ => "application/octet-stream",
            };
            if ct.get(name).is_none() && !ext.is_empty() {
                ct.defaults.push((ext, mime.into()));
            }
        }
        self.pkg.set_content_types(&ct);
        Ok(())
    }
}

// ---------------------------------------------------------------- property serialization

/// The main part's text before the body with its §17.2.1 background (the page
/// colour, first child of `w:document`) replaced by `color` (`None` removes it).
fn with_background(prefix: &str, color: Option<bo_model::Color>) -> String {
    let mut s = remove_empty_element(prefix, "w:background");
    if let (Some(c), Some(at)) = (color, find_element(&s, "w:body")) {
        s.insert_str(at, &format!("<w:background w:color=\"{}\"/>", c.to_hex()));
    }
    s
}

/// Add `xmlns:{prefix}` to a root start tag that does not declare it.
fn with_namespace(root: &str, prefix: &str, ns: &str) -> String {
    let decl = format!("xmlns:{prefix}=");
    if root.contains(&decl) {
        return root.to_owned();
    }
    match root.rfind('>') {
        Some(i) => format!("{} xmlns:{prefix}=\"{ns}\"{}", &root[..i], &root[i..]),
        None => root.to_owned(),
    }
}

/// Give the paragraph start tag of `xml` a `w14:paraId` when it has none.
fn with_para_id(xml: &str, pid: &str) -> String {
    let Some(end) = xml.find('>') else {
        return xml.to_owned();
    };
    let tag = &xml[..end];
    if tag.contains(":paraId=") {
        return xml.to_owned();
    }
    let at = if tag.ends_with('/') { end - 1 } else { end };
    format!(
        "{} w14:paraId=\"{}\"{}",
        &xml[..at],
        escape(pid),
        &xml[at..]
    )
}

/// CT_PPrBase element order (ECMA-376 Part 1, §17.3.1.26).
const PPR_ORDER: &[&str] = &[
    "pStyle",
    "keepNext",
    "keepLines",
    "pageBreakBefore",
    "framePr",
    "widowControl",
    "numPr",
    "suppressLineNumbers",
    "pBdr",
    "shd",
    "tabs",
    "suppressAutoHyphens",
    "kinsoku",
    "wordWrap",
    "overflowPunct",
    "topLinePunct",
    "autoSpaceDE",
    "autoSpaceDN",
    "bidi",
    "adjustRightInd",
    "snapToGrid",
    "spacing",
    "ind",
    "contextualSpacing",
    "mirrorIndents",
    "suppressOverlap",
    "jc",
    "textDirection",
    "textAlignment",
    "textboxTightWrap",
    "outlineLvl",
    "divId",
    "cnfStyle",
    "rPr",
    "sectPr",
    "pPrChange",
];

/// CT_RPrBase element order (§17.3.2.28).
const RPR_ORDER: &[&str] = &[
    "rStyle",
    "rFonts",
    "b",
    "bCs",
    "i",
    "iCs",
    "caps",
    "smallCaps",
    "strike",
    "dstrike",
    "outline",
    "shadow",
    "emboss",
    "imprint",
    "noProof",
    "snapToGrid",
    "vanish",
    "webHidden",
    "color",
    "spacing",
    "w",
    "kern",
    "position",
    "sz",
    "szCs",
    "highlight",
    "u",
    "effect",
    "bdr",
    "shd",
    "fitText",
    "vertAlign",
    "rtl",
    "cs",
    "em",
    "lang",
    "eastAsianLayout",
    "specVanish",
    "oMath",
    "rPrChange",
];

/// CT_TblPrBase order (§17.4.60).
const TBLPR_ORDER: &[&str] = &[
    "tblStyle",
    "tblpPr",
    "tblOverlap",
    "bidiVisual",
    "tblStyleRowBandSize",
    "tblStyleColBandSize",
    "tblW",
    "jc",
    "tblCellSpacing",
    "tblInd",
    "tblBorders",
    "shd",
    "tblLayout",
    "tblCellMar",
    "tblLook",
    "tblCaption",
    "tblDescription",
    "tblPrChange",
];

/// CT_TcPr order (§17.4.70).
const TCPR_ORDER: &[&str] = &[
    "cnfStyle",
    "tcW",
    "gridSpan",
    "hMerge",
    "vMerge",
    "tcBorders",
    "shd",
    "noWrap",
    "tcMar",
    "textDirection",
    "tcFitText",
    "vAlign",
    "hideMark",
    "headers",
    "cellIns",
    "cellDel",
    "cellMerge",
    "tcPrChange",
];

/// Local name of the root element of a raw XML fragment (`<w:foo …>` → `foo`).
fn raw_local_name(xml: &str) -> String {
    let t = xml.trim_start();
    let t = t.strip_prefix('<').unwrap_or(t);
    let end = t
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .unwrap_or(t.len());
    let q = &t[..end];
    q.rsplit(':').next().unwrap_or(q).to_owned()
}

fn raw_items(raws: &[Raw]) -> Vec<(String, String)> {
    raws.iter()
        .map(|r| (raw_local_name(&r.xml), r.xml.to_string()))
        .collect()
}

/// Join children sorted by their position in `order` (unknown names go last, stable).
fn ordered(order: &[&str], mut items: Vec<(String, String)>) -> String {
    items.sort_by_key(|(n, _)| order.iter().position(|o| o == n).unwrap_or(order.len()));
    items.into_iter().map(|(_, x)| x).collect()
}

/// A footnotePr / endnotePr element (empty string when nothing is set). Section-wide
/// and document-wide (§17.11.11, §17.11.4) share the order of CT_FtnProps /
/// CT_EdnProps: pos, numFmt, numStart, numRestart.
fn note_pr_xml(tag: &str, np: &bo_model::NoteProps) -> String {
    let mut s = String::new();
    if np.is_empty() {
        return s;
    }
    let _ = write!(s, "<w:{tag}>");
    if let Some(p) = np.pos {
        let _ = write!(s, "<w:pos w:val=\"{}\"/>", p.as_str());
    }
    if let Some(f) = &np.fmt {
        let _ = write!(s, "<w:numFmt w:val=\"{}\"/>", escape(f.as_str()));
    }
    if let Some(n) = np.start {
        let _ = write!(s, "<w:numStart w:val=\"{n}\"/>");
    }
    if let Some(r) = np.restart {
        let _ = write!(s, "<w:numRestart w:val=\"{}\"/>", r.as_str());
    }
    let _ = write!(s, "</w:{tag}>");
    s
}

/// §17.6.10 pgBorders: attributes `w:zOrder`, `w:display`, `w:offsetFrom` (written
/// when not the default) and the edges in schema order top, left, bottom, right
/// (§17.6.21, §17.6.7, §17.6.2, §17.6.15), each a `CT_Border` (§17.18.2 `ST_Border`
/// `w:val`, `w:sz`, `w:space`, `w:color`, `w:shadow`, `w:frame`).
fn page_borders_xml(pb: &bo_model::PageBorders) -> String {
    let mut s = String::from("<w:pgBorders");
    if pb.behind {
        s.push_str(" w:zOrder=\"back\"");
    }
    if pb.display != bo_model::PageBorderDisplay::AllPages {
        let _ = write!(s, " w:display=\"{}\"", pb.display.as_str());
    }
    if pb.offset_from_page {
        s.push_str(" w:offsetFrom=\"page\"");
    }
    s.push('>');
    for (tag, e) in ["top", "left", "bottom", "right"].iter().zip(pb.edges()) {
        let Some(e) = e else { continue };
        let b = &e.line;
        let _ = write!(
            s,
            "<w:{tag} w:val=\"{}\" w:sz=\"{}\" w:space=\"{}\" w:color=\"{}\"{}{}/>",
            escape(&b.style),
            b.size,
            b.space.min(31),
            b.color
                .map_or_else(|| "auto".to_owned(), bo_model::Color::to_hex),
            if e.shadow { " w:shadow=\"1\"" } else { "" },
            if e.frame { " w:frame=\"1\"" } else { "" },
        );
    }
    s.push_str("</w:pgBorders>");
    s
}

/// `CT_SectPrBase` children after the header/footer references (§17.6.17: footnotePr …
/// titlePg), in schema order.
fn sect_props_xml(sec: &Section) -> String {
    let mut s = String::new();
    s.push_str(&note_pr_xml("footnotePr", &sec.footnote_pr));
    s.push_str(&note_pr_xml("endnotePr", &sec.endnote_pr));
    let bt = match sec.break_type {
        SectionBreak::NextPage => None,
        SectionBreak::Continuous => Some("continuous"),
        SectionBreak::EvenPage => Some("evenPage"),
        SectionBreak::OddPage => Some("oddPage"),
        SectionBreak::NextColumn => Some("nextColumn"),
    };
    if let Some(t) = bt {
        let _ = write!(s, "<w:type w:val=\"{t}\"/>");
    }
    let p = &sec.page;
    let orient = if p.is_landscape() {
        " w:orient=\"landscape\""
    } else {
        ""
    };
    let _ = write!(
            s,
            "<w:pgSz w:w=\"{}\" w:h=\"{}\"{orient}/><w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"{}\" w:footer=\"{}\" w:gutter=\"0\"/>",
            p.width.0, p.height.0, p.margin_top.0, p.margin_right.0, p.margin_bottom.0, p.margin_left.0, p.header.0, p.footer.0
        );
    if let Some(pb) = &sec.page_borders {
        s.push_str(&page_borders_xml(pb));
    }
    // §17.6.8 lnNumType (countBy, start, distance, restart).
    if let Some(ln) = &sec.line_numbers {
        let _ = write!(s, "<w:lnNumType w:countBy=\"{}\"", ln.count_by.max(1));
        if ln.start > 0 {
            let _ = write!(s, " w:start=\"{}\"", ln.start);
        }
        if let Some(d) = ln.distance {
            let _ = write!(s, " w:distance=\"{}\"", d.0.max(0));
        }
        let _ = write!(s, " w:restart=\"{}\"/>", ln.restart.as_str());
    }
    if let Some(st) = sec.page_number_start {
        let _ = write!(s, "<w:pgNumType w:start=\"{st}\"/>");
    }
    s.push_str(&cols_xml(&sec.columns));
    if sec.title_page {
        s.push_str("<w:titlePg/>");
    }
    // §17.6.20 textDirection (after titlePg in CT_SectPrBase).
    if let Some(d) = sec.text_direction.filter(|d| d.rotation() != 0) {
        let _ = write!(s, "<w:textDirection w:val=\"{}\"/>", d.as_str());
    }
    s
}

/// §17.6.4 cols: @num defaults to 1, @space to 720; `w:sep` draws a line between the
/// columns; unequal columns are `w:equalWidth="0"` with one §17.6.3 `w:col` each.
fn cols_xml(c: &bo_model::Columns) -> String {
    let sep = if c.separator { " w:sep=\"1\"" } else { "" };
    if c.unequal() {
        let mut s = format!(
            "<w:cols w:num=\"{}\" w:space=\"{}\"{sep} w:equalWidth=\"0\">",
            c.cols.len(),
            c.space.0.max(0)
        );
        let n = c.cols.len();
        for (i, d) in c.cols.iter().enumerate() {
            let _ = write!(s, "<w:col w:w=\"{}\"", d.width.0.max(0));
            if i + 1 < n {
                let _ = write!(s, " w:space=\"{}\"", d.space.0.max(0));
            }
            s.push_str("/>");
        }
        s.push_str("</w:cols>");
        return s;
    }
    if c.count > 1 {
        format!(
            "<w:cols w:num=\"{}\" w:space=\"{}\"{sep}/>",
            c.count, c.space.0
        )
    } else if c.space.0 != 720 || c.separator {
        format!("<w:cols w:space=\"{}\"{sep}/>", c.space.0)
    } else {
        String::new()
    }
}

/// Children of a §17.4.70 tcPr (with §17.13.5.36 tcPrChange), unordered.
fn tcpr_items(p: &bo_model::CellProps) -> Vec<(String, String)> {
    let mut items: Vec<(String, String)> = Vec::new();
    if let Some(w) = p.width {
        items.push(("tcW".into(), width_xml("tcW", w)));
    }
    if p.grid_span > 1 {
        items.push((
            "gridSpan".into(),
            format!("<w:gridSpan w:val=\"{}\"/>", p.grid_span),
        ));
    }
    if let Some(v) = p.v_merge {
        items.push((
            "vMerge".into(),
            match v {
                VMerge::Restart => "<w:vMerge w:val=\"restart\"/>".into(),
                VMerge::Continue => "<w:vMerge/>".into(),
            },
        ));
    }
    if let Some(b) = &p.borders {
        items.push(("tcBorders".into(), borders_xml("tcBorders", b, false)));
    }
    if let Some(col) = p.shading {
        items.push((
            "shd".into(),
            format!(
                "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>",
                col.to_hex()
            ),
        ));
    }
    // ECMA-376 Part 1, §17.4.72 textDirection.
    if let Some(d) = p.text_direction {
        items.push((
            "textDirection".into(),
            format!("<w:textDirection w:val=\"{}\"/>", d.as_str()),
        ));
    }
    if let Some(v) = p.v_align {
        let v = match v {
            CellVAlign::Top => "top",
            CellVAlign::Center => "center",
            CellVAlign::Bottom => "bottom",
        };
        items.push(("vAlign".into(), format!("<w:vAlign w:val=\"{v}\"/>")));
    }
    items.extend(raw_items(&p.unknown));
    // §17.13.5.4 cellIns / §17.13.5.3 cellDel / §17.13.5.6 cellMerge (one of them).
    if let Some(m) = &p.ins {
        items.push((
            "cellIns".into(),
            format!("<w:cellIns{}/>", rev_attrs(&m.info)),
        ));
    } else if let Some(m) = &p.del {
        items.push((
            "cellDel".into(),
            format!("<w:cellDel{}/>", rev_attrs(&m.info)),
        ));
    } else if let Some(m) = &p.merge {
        // §17.18.1 ST_AnnotationVMerge
        let vm = |v: Option<VMerge>| match v {
            Some(VMerge::Restart) => "rest",
            Some(VMerge::Continue) => "cont",
            None => "",
        };
        let mut a = rev_attrs(&m.info);
        if m.v_merge.is_some() {
            let _ = write!(a, " w:vMerge=\"{}\"", vm(m.v_merge));
        }
        if m.v_merge_orig.is_some() {
            let _ = write!(a, " w:vMergeOrig=\"{}\"", vm(m.v_merge_orig));
        }
        items.push(("cellMerge".into(), format!("<w:cellMerge{a}/>")));
    }
    if let Some(ch) = &p.change {
        let old = ordered(TCPR_ORDER, tcpr_items(&ch.old.bare()));
        items.push((
            "tcPrChange".into(),
            format!(
                "<w:tcPrChange{}><w:tcPr>{old}</w:tcPr></w:tcPrChange>",
                rev_attrs(&ch.info)
            ),
        ));
    }
    items
}

thread_local! {
    /// Revision ids written so far and the counter for fresh ones (see [`unique_id`]).
    static WRITTEN_IDS: std::cell::RefCell<(std::collections::HashSet<i32>, i32)> =
        std::cell::RefCell::new((std::collections::HashSet::new(), 0));
}

/// Forget the revision ids of the previous [`write_docx`].
fn reset_ids() {
    WRITTEN_IDS.with(|c| *c.borrow_mut() = (std::collections::HashSet::new(), 900_000_000));
}

/// A revision id unique in the part set being written: annotation ids must be
/// unique (ECMA-376 Part 1, §17.13.1 `w:id`), but one edit marks several elements
/// (cells of a column, paragraphs of a deletion) with one revision. The first element
/// keeps the id, later ones get fresh ids.
fn unique_id(id: i32) -> i32 {
    WRITTEN_IDS.with(|c| {
        let mut c = c.borrow_mut();
        if c.0.insert(id) {
            return id;
        }
        loop {
            c.1 += 1;
            let n = c.1;
            if c.0.insert(n) {
                return n;
            }
        }
    })
}

/// `w:id`, `w:author`, `w:date` of a tracked change (§17.13.5 `CT_TrackChange`).
fn rev_attrs(info: &RevisionInfo) -> String {
    let mut s = format!(
        " w:id=\"{}\" w:author=\"{}\"",
        unique_id(info.id),
        escape(&info.author)
    );
    if let Some(d) = &info.date {
        let _ = write!(s, " w:date=\"{}\"", escape(d));
    }
    s
}

/// A run-level track-change container around `inner`.
fn rev_wrap(tag: &str, info: &RevisionInfo, inner: &str) -> String {
    format!("<w:{tag}{}>{inner}</w:{tag}>", rev_attrs(info))
}

/// Paragraph-mark revisions (§17.13.5.16 ins / §17.13.5.15 del in `w:pPr/w:rPr`).
fn mark_revisions(p: &ParaProps) -> String {
    let mut s = String::new();
    for (m, ins) in [(&p.mark_ins, true), (&p.mark_del, false)] {
        if let Some(m) = m {
            let tag = match (ins, m.moved) {
                (true, false) => "ins",
                (true, true) => "moveTo",
                (false, false) => "del",
                (false, true) => "moveFrom",
            };
            let _ = write!(s, "<w:{tag}{}/>", rev_attrs(&m.info));
        }
    }
    s
}

fn flag(name: &str, v: Option<bool>) -> Option<(String, String)> {
    v.map(|b| {
        (
            name.to_owned(),
            if b {
                format!("<w:{name}/>")
            } else {
                format!("<w:{name} w:val=\"0\"/>")
            },
        )
    })
}

fn jc(a: Alignment) -> &'static str {
    match a {
        Alignment::Left => "left",
        Alignment::Center => "center",
        Alignment::Right => "right",
        Alignment::Justify => "both",
    }
}

fn border_xml(tag: &str, b: &Border) -> String {
    format!(
        "<w:{tag} w:val=\"{}\" w:sz=\"{}\" w:space=\"{}\" w:color=\"{}\"/>",
        escape(&b.style),
        b.size,
        b.space,
        b.color.map_or("auto".to_owned(), bo_model::Color::to_hex)
    )
}

fn borders_xml(container: &str, b: &Borders, paragraph: bool) -> String {
    let mut s = format!("<w:{container}>");
    let edges: [(&str, &Option<Border>); 6] = [
        ("top", &b.top),
        ("left", &b.left),
        ("bottom", &b.bottom),
        ("right", &b.right),
        (if paragraph { "between" } else { "insideH" }, &b.inside_h),
        ("insideV", &b.inside_v),
    ];
    for (tag, e) in edges {
        if paragraph && tag == "insideV" {
            continue;
        }
        if let Some(e) = e {
            s.push_str(&border_xml(tag, e));
        }
    }
    let _ = write!(s, "</w:{container}>");
    s
}

fn width_xml(tag: &str, w: TableWidth) -> String {
    match w {
        TableWidth::Auto => format!("<w:{tag} w:w=\"0\" w:type=\"auto\"/>"),
        TableWidth::Dxa(t) => format!("<w:{tag} w:w=\"{}\" w:type=\"dxa\"/>", t.0),
        TableWidth::Pct(p) => format!("<w:{tag} w:w=\"{p}\" w:type=\"pct\"/>"),
    }
}

fn ppr_children(p: &ParaProps) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    if let Some(s) = &p.style {
        v.push((
            "pStyle".into(),
            format!("<w:pStyle w:val=\"{}\"/>", escape(s)),
        ));
    }
    v.extend(flag("keepNext", p.keep_next));
    v.extend(flag("keepLines", p.keep_lines));
    v.extend(flag("pageBreakBefore", p.page_break_before));
    // §17.3.1.11 framePr of a drop cap (§17.18.20 ST_DropCap): the frame wraps the
    // following text and is anchored to it (§17.3.1.11 wrap, vAnchor, hAnchor).
    if let Some(f) = &p.frame {
        let hs = if f.h_space.0 > 0 {
            format!(" w:hSpace=\"{}\"", f.h_space.0)
        } else {
            String::new()
        };
        v.push((
            "framePr".into(),
            format!(
                "<w:framePr w:dropCap=\"{}\" w:lines=\"{}\"{hs} w:wrap=\"around\" w:vAnchor=\"text\" w:hAnchor=\"{}\"/>",
                f.drop_cap.as_str(),
                f.lines.clamp(1, 10),
                if f.drop_cap == bo_model::DropCap::Margin {
                    "page"
                } else {
                    "text"
                }
            ),
        ));
    }
    v.extend(flag("widowControl", p.widow_control));
    // §17.3.1.35 suppressLineNumbers / §17.3.1.34 suppressAutoHyphens
    v.extend(flag("suppressLineNumbers", p.suppress_line_numbers));
    v.extend(flag("suppressAutoHyphens", p.suppress_auto_hyphens));
    if let Some(n) = p.num {
        v.push((
            "numPr".into(),
            format!(
                "<w:numPr><w:ilvl w:val=\"{}\"/><w:numId w:val=\"{}\"/></w:numPr>",
                n.ilvl, n.num_id
            ),
        ));
    }
    if let Some(b) = &p.borders {
        v.push(("pBdr".into(), borders_xml("pBdr", b, true)));
    }
    if let Some(c) = p.shading {
        v.push((
            "shd".into(),
            format!(
                "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>",
                c.to_hex()
            ),
        ));
    }
    if let Some(tabs) = &p.tabs {
        let mut s = String::from("<w:tabs>");
        for t in tabs {
            let a = match t.align {
                TabAlign::Left => "left",
                TabAlign::Center => "center",
                TabAlign::Right => "right",
                TabAlign::Decimal => "decimal",
                TabAlign::Bar => "bar",
                TabAlign::Clear => "clear",
            };
            let l = match t.leader {
                TabLeader::None => "",
                TabLeader::Dot => " w:leader=\"dot\"",
                TabLeader::Hyphen => " w:leader=\"hyphen\"",
                TabLeader::Underscore => " w:leader=\"underscore\"",
            };
            let _ = write!(s, "<w:tab w:val=\"{a}\"{l} w:pos=\"{}\"/>", t.pos.0);
        }
        s.push_str("</w:tabs>");
        if !tabs.is_empty() {
            v.push(("tabs".into(), s));
        }
    }
    if p.space_before.is_some() || p.space_after.is_some() || p.line_spacing.is_some() {
        let mut s = String::from("<w:spacing");
        if let Some(x) = p.space_before {
            let _ = write!(s, " w:before=\"{}\"", x.0);
        }
        if let Some(x) = p.space_after {
            let _ = write!(s, " w:after=\"{}\"", x.0);
        }
        match p.line_spacing {
            Some(LineSpacing::Auto(x)) => {
                let _ = write!(s, " w:line=\"{x}\" w:lineRule=\"auto\"");
            }
            Some(LineSpacing::Exact(x)) => {
                let _ = write!(s, " w:line=\"{}\" w:lineRule=\"exact\"", x.0);
            }
            Some(LineSpacing::AtLeast(x)) => {
                let _ = write!(s, " w:line=\"{}\" w:lineRule=\"atLeast\"", x.0);
            }
            None => {}
        }
        s.push_str("/>");
        v.push(("spacing".into(), s));
    }
    if p.indent_left.is_some() || p.indent_right.is_some() || p.indent_first_line.is_some() {
        let mut s = String::from("<w:ind");
        if let Some(x) = p.indent_left {
            let _ = write!(s, " w:left=\"{}\"", x.0);
        }
        if let Some(x) = p.indent_right {
            let _ = write!(s, " w:right=\"{}\"", x.0);
        }
        match p.indent_first_line {
            Some(x) if x.0 < 0 => {
                let _ = write!(s, " w:hanging=\"{}\"", -x.0);
            }
            Some(x) => {
                let _ = write!(s, " w:firstLine=\"{}\"", x.0);
            }
            None => {}
        }
        s.push_str("/>");
        v.push(("ind".into(), s));
    }
    v.extend(flag("contextualSpacing", p.contextual_spacing));
    v.extend(flag("bidi", p.bidi)); // §17.3.1.6
    if let Some(a) = p.align {
        v.push(("jc".into(), format!("<w:jc w:val=\"{}\"/>", jc(a))));
    }
    if let Some(l) = p.outline_level {
        v.push((
            "outlineLvl".into(),
            format!("<w:outlineLvl w:val=\"{l}\"/>"),
        ));
    }
    let marks = mark_revisions(p);
    let mut has_rpr = false;
    for (name, xml) in raw_items(&p.unknown) {
        if name == "rPr" && !marks.is_empty() {
            // CT_ParaRPr: ins/del/moveFrom/moveTo come first (§17.3.1.29).
            has_rpr = true;
            let xml = match xml.find('>') {
                Some(i) if xml[..i].ends_with('/') => {
                    format!("{}>{marks}</w:rPr>", &xml[..i - 1])
                }
                Some(i) => format!("{}{marks}{}", &xml[..=i], &xml[i + 1..]),
                None => xml,
            };
            v.push((name, xml));
        } else {
            v.push((name, xml));
        }
    }
    if !has_rpr && !marks.is_empty() {
        v.push(("rPr".into(), format!("<w:rPr>{marks}</w:rPr>")));
    }
    if let Some(c) = &p.change {
        // §17.13.5.29 pPrChange holds a CT_PPrBase (no rPr / sectPr / pPrChange).
        let mut old = c.old.clone();
        old.change = None;
        old.mark_ins = None;
        old.mark_del = None;
        let inner: Vec<(String, String)> = ppr_children(&old)
            .into_iter()
            .filter(|(n, _)| !matches!(n.as_str(), "rPr" | "sectPr" | "pPrChange"))
            .collect();
        v.push((
            "pPrChange".into(),
            format!(
                "<w:pPrChange{}><w:pPr>{}</w:pPr></w:pPrChange>",
                rev_attrs(&c.info),
                ordered(PPR_ORDER, inner)
            ),
        ));
    }
    v
}

fn rpr_children(p: &RunProps) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    if let Some(s) = &p.style {
        v.push((
            "rStyle".into(),
            format!("<w:rStyle w:val=\"{}\"/>", escape(s)),
        ));
    }
    // §17.3.2.26 rFonts: an ascii font without an hAnsi font is written for both.
    let (h_ansi, h_theme) = if p.font_h_ansi.is_none() && p.theme_h_ansi.is_none() {
        (p.font.as_ref(), p.theme_ascii.as_ref())
    } else {
        (p.font_h_ansi.as_ref(), p.theme_h_ansi.as_ref())
    };
    let attrs = [
        ("ascii", p.font.as_ref()),
        ("hAnsi", h_ansi),
        ("eastAsia", p.font_east_asia.as_ref()),
        ("cs", p.font_cs.as_ref()),
        ("asciiTheme", p.theme_ascii.as_ref()),
        ("hAnsiTheme", h_theme),
        ("eastAsiaTheme", p.theme_east_asia.as_ref()),
        ("cstheme", p.theme_cs.as_ref()),
        ("hint", p.font_hint.as_ref()),
    ];
    if attrs.iter().any(|(_, v)| v.is_some()) {
        let mut s = String::from("<w:rFonts");
        for (k, val) in attrs {
            if let Some(f) = val {
                let _ = write!(s, " w:{k}=\"{}\"", escape(f));
            }
        }
        s.push_str("/>");
        v.push(("rFonts".into(), s));
    }
    v.extend(flag("b", p.bold));
    v.extend(flag("i", p.italic));
    v.extend(flag("caps", p.caps));
    v.extend(flag("smallCaps", p.small_caps));
    v.extend(flag("strike", p.strike));
    v.extend(flag("dstrike", p.double_strike));
    v.extend(flag("vanish", p.hidden));
    if let Some(c) = p.color {
        v.push((
            "color".into(),
            format!("<w:color w:val=\"{}\"/>", c.to_hex()),
        ));
    }
    if let Some(s) = p.spacing {
        v.push(("spacing".into(), format!("<w:spacing w:val=\"{}\"/>", s.0)));
    }
    if let Some(k) = p.kern {
        v.push(("kern".into(), format!("<w:kern w:val=\"{}\"/>", k.0)));
    }
    if let Some(sz) = p.size {
        v.push(("sz".into(), format!("<w:sz w:val=\"{}\"/>", sz.0)));
    }
    if let Some(h) = p.highlight {
        v.push((
            "highlight".into(),
            format!("<w:highlight w:val=\"{}\"/>", h.highlight_name()),
        ));
    }
    if let Some(u) = p.underline {
        // §17.3.2.40 u, §17.18.99 ST_Underline
        v.push(("u".into(), format!("<w:u w:val=\"{}\"/>", u.as_str())));
    }
    if let Some(c) = p.shading {
        v.push((
            "shd".into(),
            format!(
                "<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>",
                c.to_hex()
            ),
        ));
    }
    if let Some(va) = p.vert_align {
        let val = match va {
            VertAlign::Baseline => "baseline",
            VertAlign::Superscript => "superscript",
            VertAlign::Subscript => "subscript",
        };
        v.push((
            "vertAlign".into(),
            format!("<w:vertAlign w:val=\"{val}\"/>"),
        ));
    }
    v.extend(flag("rtl", p.rtl)); // §17.3.2.30
    v.extend(flag("cs", p.complex)); // §17.3.2.7
    v.extend(raw_items(&p.unknown));
    if let Some(c) = &p.change {
        // §17.13.5.31 rPrChange holds a CT_RPrOriginal (no rPrChange of its own).
        let inner: Vec<(String, String)> = rpr_children(&c.old.without_revisions())
            .into_iter()
            .filter(|(n, _)| n != "rPrChange")
            .collect();
        v.push((
            "rPrChange".into(),
            format!(
                "<w:rPrChange{}><w:rPr>{}</w:rPr></w:rPrChange>",
                rev_attrs(&c.info),
                ordered(RPR_ORDER, inner)
            ),
        ));
    }
    v
}

fn tblpr_children(t: &TableProps) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    // §17.13.5.34 tblPrChange: the previous table properties.
    if let Some(ch) = &t.change {
        v.push((
            "tblPrChange".into(),
            format!(
                "<w:tblPrChange{}><w:tblPr>{}</w:tblPr></w:tblPrChange>",
                rev_attrs(&ch.info),
                ordered(TBLPR_ORDER, tblpr_children(&ch.old))
            ),
        ));
    }
    if let Some(s) = &t.style {
        v.push((
            "tblStyle".into(),
            format!("<w:tblStyle w:val=\"{}\"/>", escape(s)),
        ));
    }
    if let Some(w) = t.width {
        v.push(("tblW".into(), width_xml("tblW", w)));
    }
    if let Some(a) = t.align {
        v.push(("jc".into(), format!("<w:jc w:val=\"{}\"/>", jc(a))));
    }
    if let Some(i) = t.indent {
        v.push((
            "tblInd".into(),
            format!("<w:tblInd w:w=\"{}\" w:type=\"dxa\"/>", i.0),
        ));
    }
    if let Some(b) = &t.borders {
        v.push(("tblBorders".into(), borders_xml("tblBorders", b, false)));
    }
    if t.fixed_layout {
        v.push(("tblLayout".into(), "<w:tblLayout w:type=\"fixed\"/>".into()));
    }
    // ECMA-376 Part 1, §17.4.58 tblpPr (floating table position).
    if let Some(f) = &t.float {
        let mut x = format!(
            "<w:tblpPr w:leftFromText=\"{}\" w:rightFromText=\"{}\" w:topFromText=\"{}\" w:bottomFromText=\"{}\"",
            f.left_from_text.0, f.right_from_text.0, f.top_from_text.0, f.bottom_from_text.0
        );
        let mut opt = |name: &str, v: &Option<String>| {
            if let Some(v) = v {
                let _ = write!(x, " w:{name}=\"{}\"", escape(v));
            }
        };
        opt("vertAnchor", &f.vert_anchor);
        opt("horzAnchor", &f.horz_anchor);
        opt("tblpXSpec", &f.x_align);
        if f.x_align.is_none() {
            let _ = write!(x, " w:tblpX=\"{}\"", f.x.0);
        }
        if let Some(ya) = &f.y_align {
            let _ = write!(x, " w:tblpYSpec=\"{}\"", escape(ya));
        } else {
            let _ = write!(x, " w:tblpY=\"{}\"", f.y.0);
        }
        x.push_str("/>");
        v.push(("tblpPr".into(), x));
    }
    if let Some((l, r)) = t.cell_margin {
        v.push((
            "tblCellMar".into(),
            format!("<w:tblCellMar><w:left w:w=\"{}\" w:type=\"dxa\"/><w:right w:w=\"{}\" w:type=\"dxa\"/></w:tblCellMar>", l.0, r.0),
        ));
    }
    v.extend(raw_items(&t.unknown));
    v
}

/// §17.5.2.29 sdt around paragraphs: each block-level start marker in `b` opens one before it.
fn open_block_controls(out: &mut String, b: &Block, depth: &mut usize) {
    let Block::Paragraph(p) = b else { return };
    for c in p.runs.iter().flat_map(|r| &r.content) {
        if let RunContent::ControlStart(cc) = c {
            if cc.block {
                let _ = write!(out, "<w:sdt>{}<w:sdtContent>", sdt_pr(cc));
                *depth += 1;
            }
        }
    }
}

/// Each block-level end marker in `b` closes an open control after it (an unmatched one is
/// dropped, so edits that remove a start marker still give well-formed XML).
fn close_block_controls(out: &mut String, b: &Block, depth: &mut usize) {
    let Block::Paragraph(p) = b else { return };
    for c in p.runs.iter().flat_map(|r| &r.content) {
        if matches!(c, RunContent::ControlEnd(true)) && *depth > 0 {
            out.push_str("</w:sdtContent></w:sdt>");
            *depth -= 1;
        }
    }
}

/// Close the controls still open at the end of a container.
fn close_all_block_controls(out: &mut String, depth: &mut usize) {
    for _ in 0..*depth {
        out.push_str("</w:sdtContent></w:sdt>");
    }
    *depth = 0;
}

/// §17.5.2.38 sdtPr: as read while the modelled fields are unchanged, else written from them.
fn sdt_pr(cc: &ContentControl) -> String {
    if let Some(xml) = &cc.props_xml {
        let mut read = crate::reader::parse_sdt_pr(xml, cc.block);
        read.props_xml.clone_from(&cc.props_xml);
        if read == *cc {
            return xml.clone();
        }
    }
    let mut s = String::from("<w:sdtPr>");
    // CT_SdtPr order: alias, tag, id, lock, showingPlcHdr, then the kind.
    if let Some(a) = &cc.alias {
        let _ = write!(s, "<w:alias w:val=\"{}\"/>", escape(a));
    }
    if let Some(t) = &cc.tag {
        let _ = write!(s, "<w:tag w:val=\"{}\"/>", escape(t));
    }
    if let Some(id) = cc.id {
        let _ = write!(s, "<w:id w:val=\"{id}\"/>");
    }
    if let Some(l) = &cc.lock {
        let _ = write!(s, "<w:lock w:val=\"{}\"/>", escape(l));
    }
    if cc.placeholder {
        s.push_str("<w:showingPlcHdr/>");
    }
    let list = |s: &mut String, items: &[(String, String)]| {
        for (d, v) in items {
            let _ = write!(
                s,
                "<w:listItem w:displayText=\"{}\" w:value=\"{}\"/>",
                escape(d),
                escape(v)
            );
        }
    };
    match &cc.kind {
        ControlKind::Text { multi_line } => {
            if *multi_line {
                s.push_str("<w:text w:multiLine=\"1\"/>");
            } else {
                s.push_str("<w:text/>");
            }
        }
        ControlKind::RichText | ControlKind::Other => {}
        ControlKind::Picture => s.push_str("<w:picture/>"),
        ControlKind::ComboBox(items) => {
            s.push_str("<w:comboBox>");
            list(&mut s, items);
            s.push_str("</w:comboBox>");
        }
        ControlKind::DropDown(items) => {
            s.push_str("<w:dropDownList>");
            list(&mut s, items);
            s.push_str("</w:dropDownList>");
        }
        ControlKind::Date { format, full_date } => {
            match full_date {
                Some(d) => {
                    let _ = write!(s, "<w:date w:fullDate=\"{}\">", escape(d));
                }
                None => s.push_str("<w:date>"),
            }
            let _ = write!(
                s,
                "<w:dateFormat w:val=\"{}\"/><w:lid w:val=\"vi-VN\"/><w:storeMappedDataAs w:val=\"dateTime\"/><w:calendar w:val=\"gregorian\"/></w:date>",
                escape(format)
            );
        }
        // [MS-DOCX] w14:checkbox.
        ControlKind::Checkbox { checked, on, off } => {
            let _ = write!(
                s,
                "<w14:checkbox xmlns:w14=\"{W14}\"><w14:checked w14:val=\"{}\"/><w14:checkedState w14:val=\"{:04X}\" w14:font=\"MS Gothic\"/><w14:uncheckedState w14:val=\"{:04X}\" w14:font=\"MS Gothic\"/></w14:checkbox>",
                u8::from(*checked),
                u32::from(*on),
                u32::from(*off)
            );
        }
        // [MS-DOCX] w15:repeatingSection.
        ControlKind::RepeatingSection => {
            let _ = write!(s, "<w15:repeatingSection xmlns:w15=\"{W15}\"/>");
        }
        ControlKind::Group => s.push_str("<w:group/>"),
    }
    s.push_str("</w:sdtPr>");
    s
}

const W14: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
const W15: &str = "http://schemas.microsoft.com/office/word/2012/wordml";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read_docx;
    use bo_model::{
        Color, HalfPoints, NumRef, PageSetup, Paragraph, Run, Section, Style, StyleKind, Twips,
    };

    fn sample() -> Document {
        let bold = RunProps {
            bold: Some(true),
            size: Some(HalfPoints(28)),
            color: Some(Color {
                r: 0x1F,
                g: 0x4E,
                b: 0x99,
            }),
            ..Default::default()
        };
        let mut doc = Document {
            default_run: RunProps {
                font: Some("Times New Roman".into()),
                size: Some(HalfPoints(26)),
                ..Default::default()
            },
            default_para: ParaProps {
                space_after: Some(Twips(120)),
                ..Default::default()
            },
            ..Default::default()
        };
        let list = doc.numbering.add_simple_list(false);
        let mut h1 = Style::new("Heading1", "heading 1", StyleKind::Paragraph);
        h1.rpr.bold = Some(true);
        h1.rpr.size = Some(HalfPoints(32));
        doc.styles.styles.push(h1);
        doc.sections = vec![
            Section {
                headers: bo_model::HeaderFooter {
                    default: Some(vec![Block::Paragraph(Paragraph::new(
                        ParaProps::default(),
                        vec![Run {
                            props: RunProps::default(),
                            content: vec![RunContent::Field {
                                instr: "PAGE".into(),
                                cached: "1".into(),
                                locked: false,
                            }],
                        }],
                    ))]),
                    ..Default::default()
                },
                ..Section::new(vec![
                    Block::Paragraph(Paragraph::new(
                        ParaProps {
                            align: Some(Alignment::Center),
                            style: Some("Heading1".into()),
                            ..Default::default()
                        },
                        vec![Run::text("CỘNG HÒA XÃ HỘI CHỦ NGHĨA VIỆT NAM", bold)],
                    )),
                    Block::Paragraph(Paragraph::new(
                        ParaProps {
                            num: Some(NumRef {
                                num_id: list,
                                ilvl: 0,
                            }),
                            ..Default::default()
                        },
                        vec![Run::text(
                            "liên kết",
                            RunProps {
                                link: Some("https://example.vn/a?b=1&c=2".into()),
                                ..Default::default()
                            },
                        )],
                    )),
                    Block::Table(Table::simple(2, 3, Twips(9000))),
                    // A section must end with a paragraph that carries its sectPr (§17.6.17).
                    Block::Paragraph(Paragraph::default()),
                ])
            },
            Section {
                page: PageSetup {
                    width: Twips(16_838),
                    height: Twips(11_906),
                    ..PageSetup::default()
                },
                ..Section::new(vec![Block::Paragraph(Paragraph::new(
                    ParaProps {
                        align: Some(Alignment::Justify),
                        indent_first_line: Some(Twips(567)),
                        line_spacing: Some(LineSpacing::Auto(360)),
                        page_break_before: Some(true),
                        ..Default::default()
                    },
                    vec![Run {
                        props: RunProps {
                            italic: Some(true),
                            underline: Some(bo_model::Underline::Single),
                            vert_align: Some(VertAlign::Superscript),
                            highlight: Color::from_highlight("yellow"),
                            ..Default::default()
                        },
                        content: vec![
                            RunContent::Text("Độc lập & <Tự do>".into()),
                            RunContent::Tab,
                            RunContent::LineBreak,
                            RunContent::PageBreak,
                        ],
                    }],
                ))])
            },
        ];
        doc
    }

    fn strip(mut d: Document) -> Document {
        fn blocks(v: &mut [Block]) {
            for b in v {
                match b {
                    Block::Paragraph(p) => p.src = None,
                    Block::Table(t) => {
                        t.src = None;
                        for r in &mut t.rows {
                            for c in &mut r.cells {
                                blocks(&mut c.blocks);
                            }
                        }
                    }
                    Block::Opaque(_) => {}
                }
            }
        }
        d.source = None;
        // A new document gets settings.xml with the default tab stop (720 twips).
        if d.default_tab_stop == Some(Twips(720)) {
            d.default_tab_stop = None;
        }
        for s in &mut d.sections {
            s.src = None;
            blocks(&mut s.blocks);
            for set in [&mut s.headers, &mut s.footers] {
                for v in [&mut set.default, &mut set.first, &mut set.even]
                    .into_iter()
                    .flatten()
                {
                    blocks(v);
                }
            }
        }
        for st in &mut d.styles.styles {
            st.src = None;
        }
        d
    }

    #[test]
    fn linked_list_levels_and_start_overrides_round_trip() {
        // §17.9.23 pStyle on a level, the style's numPr, §17.9.26 startOverride.
        let mut doc = sample();
        let id = doc.numbering.nums[0].id;
        doc.numbering.abstracts[0].levels[0].pstyle = Some("Heading1".into());
        doc.numbering.nums.push(bo_model::Num {
            id: id + 1,
            abstract_id: doc.numbering.nums[0].abstract_id,
            start_overrides: vec![(0, 7)],
        });
        let h1 = doc
            .styles
            .styles
            .iter_mut()
            .find(|s| s.id == "Heading1")
            .unwrap();
        h1.ppr.num = Some(NumRef {
            num_id: id,
            ilvl: 0,
        });
        let bytes = write_docx(&doc).unwrap();
        let back = strip(read_docx(&bytes).unwrap());
        assert_eq!(back.numbering, doc.numbering);
        assert_eq!(back.styles, doc.styles);
        let pkg = Package::read(&bytes, &Limits::default()).unwrap();
        let xml = String::from_utf8(pkg.part("/word/numbering.xml").unwrap().data.clone()).unwrap();
        // CT_Lvl order: start, numFmt, (lvlRestart), pStyle, …, lvlText.
        let at = |s: &str| xml.find(s).unwrap();
        assert!(at("<w:numFmt") < at("<w:pStyle w:val=\"Heading1\"/>"));
        assert!(at("<w:pStyle w:val=\"Heading1\"/>") < at("<w:lvlText"));
        assert!(xml.contains("<w:startOverride w:val=\"7\"/>"));
    }

    #[test]
    fn write_then_read_is_identity() {
        let doc = sample();
        let bytes = write_docx(&doc).unwrap();
        let back = strip(read_docx(&bytes).unwrap());
        for (a, b) in back.sections.iter().zip(&doc.sections) {
            for (x, y) in a.blocks.iter().zip(&b.blocks) {
                assert_eq!(x, y);
            }
            assert_eq!(a.headers, b.headers);
            assert_eq!(a.page, b.page);
        }
        assert_eq!(back.styles, doc.styles);
        assert_eq!(back.numbering, doc.numbering);
        assert_eq!(back.default_run, doc.default_run);
        assert_eq!(back.default_para, doc.default_para);
        assert_eq!(back.footnotes, doc.footnotes);
        assert_eq!(back.media.len(), doc.media.len());
        assert_eq!(back.even_and_odd_headers, doc.even_and_odd_headers);
        assert_eq!(back.default_tab_stop, doc.default_tab_stop);
        assert_eq!(back.sections.len(), doc.sections.len());
        for (a, b) in back.sections.iter().zip(&doc.sections) {
            assert_eq!(a, b);
        }
        assert_eq!(back, doc);
    }

    #[test]
    fn unchanged_document_round_trips_byte_identical_parts() {
        let bytes = write_docx(&sample()).unwrap();
        let doc = read_docx(&bytes).unwrap();
        let again = write_docx(&doc).unwrap();
        let a = Package::read(&bytes, &Limits::default()).unwrap();
        let b = Package::read(&again, &Limits::default()).unwrap();
        for p in a.parts() {
            assert_eq!(
                b.part(&p.name).map(|x| &x.data),
                Some(&p.data),
                "part {} changed",
                p.name
            );
        }
    }

    #[test]
    fn editing_one_paragraph_keeps_others_verbatim() {
        let bytes = write_docx(&sample()).unwrap();
        let mut doc = read_docx(&bytes).unwrap();
        let orig_second = match &doc.sections[0].blocks[1] {
            Block::Paragraph(p) => p.src.clone().unwrap().xml,
            _ => panic!(),
        };
        let Block::Paragraph(p) = &mut doc.sections[0].blocks[0] else {
            panic!()
        };
        p.runs[0].content = vec![RunContent::Text("ĐÃ SỬA".into())];
        let out = write_docx(&doc).unwrap();
        let pkg = Package::read(&out, &Limits::default()).unwrap();
        let xml = String::from_utf8(pkg.part("/word/document.xml").unwrap().data.clone()).unwrap();
        assert!(xml.contains("ĐÃ SỬA"));
        assert!(xml.contains(orig_second.as_ref()));
        assert_eq!(
            read_docx(&out).unwrap().plain_text().lines().next(),
            Some("ĐÃ SỬA")
        );
    }

    #[test]
    fn schema_order_of_unknown_children() {
        let items = vec![
            ("jc".to_owned(), "<w:jc/>".to_owned()),
            ("keepNext".to_owned(), "<w:keepNext/>".to_owned()),
            ("bidi".to_owned(), raw_local_name("<w:bidi/>")),
        ];
        let s = ordered(PPR_ORDER, items);
        assert!(s.starts_with("<w:keepNext/>"));
        assert_eq!(
            raw_local_name("  <w14:textOutline w14:w=\"1\">"),
            "textOutline"
        );
    }
}
