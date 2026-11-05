//! `bo_model::Document` → `.docx` (ECMA-376 Part 1, §17).
//!
//! Lossless strategy (ADR-0005): when the document came from a package, the
//! package is copied and only `document.xml` (plus styles/numbering/headers when
//! their model changed) is rewritten. Inside `document.xml`, every block whose
//! fingerprint matches its source is emitted verbatim; changed blocks are
//! regenerated, keeping unknown property children in schema order.

use crate::{Result, CT_DOCUMENT, CT_STYLES};
use bo_model::{
    fingerprint, Alignment, Block, Border, Borders, Cell, CellVAlign, Document, HeightRule, Image,
    LineSpacing, ParaProps, Paragraph, Raw, Run, RunContent, RunProps, Section, SectionBreak,
    StyleKind, TabAlign, TabLeader, Table, TableProps, TableWidth, Underline, VMerge, VertAlign,
    Wrap,
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
const REL_HEADER: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/header";
const REL_FOOTER: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footer";
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
}

/// Serialize a document to `.docx` bytes.
pub fn write_docx(doc: &Document) -> Result<Vec<u8>> {
    let (pkg, main) = match &doc.source {
        Some(src) => (Package::read(&src.bytes, &Limits::default())?, src.main_part.clone()),
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
    };
    w.write_main(&main)?;
    w.write_styles(&main)?;
    w.write_numbering(&main)?;
    if doc.source.is_none() {
        w.write_settings(&main)?;
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
                match b {
                    Block::Paragraph(p) if carries_sect => {
                        // The paragraph mark holds this section's sectPr (§17.6.17).
                        let para_unchanged = p
                            .src
                            .as_ref()
                            .is_some_and(|r| r.fingerprint == p.content_fingerprint());
                        if para_unchanged && sect_unchanged {
                            body.push_str(&p.src.as_ref().map(|r| r.xml.to_string()).unwrap_or_default());
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
            }
            if last {
                let sp = self.sect_pr(sec)?;
                body.push_str(&sp);
            }
        }
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
        for b in blocks {
            s.push_str(&self.block(b));
        }
        s
    }

    // ------------------------------------------------------------ paragraph

    fn paragraph(&mut self, p: &Paragraph, sect_pr: Option<&str>) -> String {
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
                let mut inner = String::new();
                while j < p.runs.len() && p.runs[j].props.link.as_ref() == Some(link) {
                    inner.push_str(&self.run(&p.runs[j]));
                    j += 1;
                }
                if let Some(anchor) = link.strip_prefix('#') {
                    let _ = write!(s, "<w:hyperlink w:anchor=\"{}\">{inner}</w:hyperlink>", escape(anchor));
                } else {
                    let id = self.rels.id_for(REL_HYPERLINK, link, true);
                    let _ = write!(s, "<w:hyperlink xmlns:r=\"{R_NS}\" r:id=\"{id}\">{inner}</w:hyperlink>");
                }
                i = j;
            } else {
                s.push_str(&self.run(run));
                i += 1;
            }
        }
        s.push_str("</w:p>");
        s
    }

    fn run(&mut self, run: &Run) -> String {
        let rpr = ordered(RPR_ORDER, rpr_children(&run.props));
        let rpr = if rpr.is_empty() {
            String::new()
        } else {
            format!("<w:rPr>{rpr}</w:rPr>")
        };
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
                    let _ = write!(cur, "<w:t xml:space=\"preserve\">{}</w:t>", escape(t));
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
                RunContent::NoteMark => cur.push_str("<w:footnoteRef/>"),
                RunContent::Opaque(r) => cur.push_str(&r.xml),
                RunContent::Image(img) => {
                    let xml = match &img.src {
                        Some(r) if !r.xml.is_empty() && !self.image_changed(img) => r.xml.to_string(),
                        _ => self.drawing(img),
                    };
                    cur.push_str(&xml);
                }
                RunContent::Field { instr, cached } => {
                    flush(&mut out, &mut cur);
                    // §17.16.19 fldSimple with cached result.
                    let _ = write!(
                        out,
                        "<w:fldSimple w:instr=\"{}\"><w:r>{rpr}<w:t xml:space=\"preserve\">{}</w:t></w:r></w:fldSimple>",
                        escape(&format!(" {instr} ")),
                        escape(cached)
                    );
                }
                RunContent::Markup(r) => {
                    flush(&mut out, &mut cur);
                    out.push_str(&r.xml);
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
    fn image_changed(&self, img: &Image) -> bool {
        let Some(src) = &img.src else { return true };
        let x = &src.xml;
        !(x.contains(&format!("cx=\"{}\"", img.width.0)) && x.contains(&format!("cy=\"{}\"", img.height.0)))
    }

    /// Generate a `w:drawing` for a picture (§20.4.2.8 inline / §20.4.2.3 anchor).
    fn drawing(&mut self, img: &Image) -> String {
        self.drawing_id += 1;
        let id = self.drawing_id;
        let rid = self.rels.id_for(REL_IMAGE, &img.media, false);
        let (cx, cy) = (img.width.0, img.height.0);
        let crop = img.crop.map_or(String::new(), |c| {
            format!("<a:srcRect l=\"{}\" t=\"{}\" r=\"{}\" b=\"{}\"/>", c[0], c[1], c[2], c[3])
        });
        let rot = if img.rotation == 0 {
            String::new()
        } else {
            format!(" rot=\"{}\"", img.rotation)
        };
        let graphic = format!(
            "<a:graphic xmlns:a=\"{A_NS}\"><a:graphicData uri=\"{PIC_NS}\"><pic:pic xmlns:pic=\"{PIC_NS}\"><pic:nvPicPr><pic:cNvPr id=\"{id}\" name=\"Picture {id}\"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip xmlns:r=\"{R_NS}\" r:embed=\"{rid}\"/>{crop}<a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm{rot}><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic>"
        );
        let descr = escape(&img.description);
        match &img.anchor {
            None => format!(
                "<w:drawing><wp:inline xmlns:wp=\"{WP_NS}\" distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\"><wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:docPr id=\"{id}\" name=\"Picture {id}\" descr=\"{descr}\"/>{graphic}</wp:inline></w:drawing>"
            ),
            Some(a) => {
                let wrap = match a.wrap {
                    Wrap::None => "<wp:wrapNone/>".to_owned(),
                    Wrap::Square => "<wp:wrapSquare wrapText=\"bothSides\"/>".to_owned(),
                    Wrap::Tight => "<wp:wrapTight wrapText=\"bothSides\"><wp:wrapPolygon edited=\"0\"><wp:start x=\"0\" y=\"0\"/><wp:lineTo x=\"0\" y=\"21600\"/><wp:lineTo x=\"21600\" y=\"21600\"/><wp:lineTo x=\"21600\" y=\"0\"/><wp:lineTo x=\"0\" y=\"0\"/></wp:wrapPolygon></wp:wrapTight>".to_owned(),
                    Wrap::TopAndBottom => "<wp:wrapTopAndBottom/>".to_owned(),
                };
                format!(
                    "<w:drawing><wp:anchor xmlns:wp=\"{WP_NS}\" distT=\"0\" distB=\"0\" distL=\"114300\" distR=\"114300\" simplePos=\"0\" relativeHeight=\"{id}\" behindDoc=\"{}\" locked=\"0\" layoutInCell=\"1\" allowOverlap=\"1\"><wp:simplePos x=\"0\" y=\"0\"/><wp:positionH relativeFrom=\"{}\"><wp:posOffset>{}</wp:posOffset></wp:positionH><wp:positionV relativeFrom=\"{}\"><wp:posOffset>{}</wp:posOffset></wp:positionV><wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/>{wrap}<wp:docPr id=\"{id}\" name=\"Picture {id}\" descr=\"{descr}\"/><wp:cNvGraphicFramePr/>{graphic}</wp:anchor></w:drawing>",
                    u8::from(a.behind),
                    escape(&a.h_relative),
                    a.x.0,
                    escape(&a.v_relative),
                    a.y.0
                )
            }
        }
    }

    // ------------------------------------------------------------ table

    fn table(&mut self, t: &Table) -> String {
        let mut s = String::from("<w:tbl>");
        let tblpr = ordered(TBLPR_ORDER, tblpr_children(&t.props));
        let _ = write!(s, "<w:tblPr>{tblpr}</w:tblPr><w:tblGrid>");
        for g in &t.grid {
            let _ = write!(s, "<w:gridCol w:w=\"{}\"/>", g.0);
        }
        s.push_str("</w:tblGrid>");
        for row in &t.rows {
            s.push_str("<w:tr>");
            let mut trpr: Vec<String> = Vec::new();
            if row.props.cant_split {
                trpr.push("<w:cantSplit/>".into());
            }
            if let Some((h, rule)) = row.props.height {
                trpr.push(format!(
                    "<w:trHeight w:val=\"{}\"{}/>",
                    h.0,
                    if rule == HeightRule::Exact { " w:hRule=\"exact\"" } else { "" }
                ));
            }
            if row.props.header {
                trpr.push("<w:tblHeader/>".into());
            }
            trpr.extend(row.props.unknown.iter().map(|r| r.xml.to_string()));
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
        let mut items: Vec<(String, String)> = Vec::new();
        if let Some(w) = c.props.width {
            items.push(("tcW".into(), width_xml("tcW", w)));
        }
        if c.props.grid_span > 1 {
            items.push(("gridSpan".into(), format!("<w:gridSpan w:val=\"{}\"/>", c.props.grid_span)));
        }
        if let Some(v) = c.props.v_merge {
            items.push((
                "vMerge".into(),
                match v {
                    VMerge::Restart => "<w:vMerge w:val=\"restart\"/>".into(),
                    VMerge::Continue => "<w:vMerge/>".into(),
                },
            ));
        }
        if let Some(b) = &c.props.borders {
            items.push(("tcBorders".into(), borders_xml("tcBorders", b, false)));
        }
        if let Some(col) = c.props.shading {
            items.push(("shd".into(), format!("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>", col.to_hex())));
        }
        if let Some(v) = c.props.v_align {
            let v = match v {
                CellVAlign::Top => "top",
                CellVAlign::Center => "center",
                CellVAlign::Bottom => "bottom",
            };
            items.push(("vAlign".into(), format!("<w:vAlign w:val=\"{v}\"/>")));
        }
        items.extend(raw_items(&c.props.unknown));
        let tcpr = ordered(TCPR_ORDER, items);
        let mut content = self.blocks(&c.blocks);
        // §17.4.66: a cell must end with a paragraph.
        if !matches!(c.blocks.last(), Some(Block::Paragraph(_)) | Some(Block::Opaque(_))) {
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
            let file = format!("{}bo{}.xml", if is_header { "header" } else { "footer" }, self.hf_counter);
            let part = format!("/word/{file}");
            let body = self.blocks(blocks);
            let body = if body.is_empty() { "<w:p/>".to_owned() } else { body };
            let xml = format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:{tag} xmlns:w=\"{W_NS}\" xmlns:r=\"{R_NS}\" xmlns:wp=\"{WP_NS}\" xmlns:a=\"{A_NS}\" xmlns:pic=\"{PIC_NS}\">{body}</w:{tag}>"
            );
            self.pkg.add_part(&part, if is_header { CT_HEADER } else { CT_FOOTER }, xml.into_bytes())?;
            let id = self.rels.id_for(if is_header { REL_HEADER } else { REL_FOOTER }, &part, false);
            let el = if is_header { "headerReference" } else { "footerReference" };
            let _ = write!(s, "<w:{el} xmlns:r=\"{R_NS}\" w:type=\"{kind}\" r:id=\"{id}\"/>");
        }
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
        let orient = if p.is_landscape() { " w:orient=\"landscape\"" } else { "" };
        let _ = write!(
            s,
            "<w:pgSz w:w=\"{}\" w:h=\"{}\"{orient}/><w:pgMar w:top=\"{}\" w:right=\"{}\" w:bottom=\"{}\" w:left=\"{}\" w:header=\"{}\" w:footer=\"{}\" w:gutter=\"0\"/>",
            p.width.0, p.height.0, p.margin_top.0, p.margin_right.0, p.margin_bottom.0, p.margin_left.0, p.header.0, p.footer.0
        );
        if let Some(st) = sec.page_number_start {
            let _ = write!(s, "<w:pgNumType w:start=\"{st}\"/>");
        }
        if sec.columns.count > 1 {
            let _ = write!(s, "<w:cols w:num=\"{}\" w:space=\"{}\"/>", sec.columns.count, sec.columns.space.0);
        }
        if sec.title_page {
            s.push_str("<w:titlePg/>");
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
                if st.is_default { " w:default=\"1\"" } else { "" },
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
                let _ = write!(s, "<w:tblPr>{}</w:tblPr>", ordered(TBLPR_ORDER, tblpr_children(t)));
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
                if l.suffix != "tab" {
                    let _ = write!(s, "<w:suff w:val=\"{}\"/>", escape(&l.suffix));
                }
                let _ = write!(s, "<w:lvlText w:val=\"{}\"/><w:lvlJc w:val=\"{}\"/>", escape(&l.text), jc(l.align));
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
            let _ = write!(s, "<w:num w:numId=\"{}\"><w:abstractNumId w:val=\"{}\"/>", n.id, n.abstract_id);
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

    fn write_settings(&mut self, _main: &str) -> Result<()> {
        let tab = self.doc.default_tab_stop.map_or(720, |t| t.0);
        let mut s = format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w:settings xmlns:w=\"{W_NS}\">");
        if self.doc.even_and_odd_headers {
            s.push_str("<w:evenAndOddHeaders/>");
        }
        let _ = write!(s, "<w:defaultTabStop w:val=\"{tab}\"/><w:compat><w:compatSetting w:name=\"compatibilityMode\" w:uri=\"http://schemas.microsoft.com/office/word\" w:val=\"15\"/></w:compat></w:settings>");
        self.pkg.add_part("/word/settings.xml", CT_SETTINGS, s.into_bytes())?;
        self.rels.id_for(REL_SETTINGS, "/word/settings.xml", false);
        Ok(())
    }

    fn write_media(&mut self) -> Result<()> {
        let mut ct = self.pkg.content_types()?;
        for (name, data) in &self.doc.media {
            if self.pkg.part(name).is_none() {
                self.pkg.put_part(name, data.as_ref().clone());
            }
            let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
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

/// CT_PPrBase element order (ECMA-376 Part 1, §17.3.1.26).
const PPR_ORDER: &[&str] = &[
    "pStyle", "keepNext", "keepLines", "pageBreakBefore", "framePr", "widowControl", "numPr",
    "suppressLineNumbers", "pBdr", "shd", "tabs", "suppressAutoHyphens", "kinsoku", "wordWrap",
    "overflowPunct", "topLinePunct", "autoSpaceDE", "autoSpaceDN", "bidi", "adjustRightInd",
    "snapToGrid", "spacing", "ind", "contextualSpacing", "mirrorIndents", "suppressOverlap", "jc",
    "textDirection", "textAlignment", "textboxTightWrap", "outlineLvl", "divId", "cnfStyle", "rPr",
    "sectPr", "pPrChange",
];

/// CT_RPrBase element order (§17.3.2.28).
const RPR_ORDER: &[&str] = &[
    "rStyle", "rFonts", "b", "bCs", "i", "iCs", "caps", "smallCaps", "strike", "dstrike", "outline",
    "shadow", "emboss", "imprint", "noProof", "snapToGrid", "vanish", "webHidden", "color",
    "spacing", "w", "kern", "position", "sz", "szCs", "highlight", "u", "effect", "bdr", "shd",
    "fitText", "vertAlign", "rtl", "cs", "em", "lang", "eastAsianLayout", "specVanish", "oMath",
    "rPrChange",
];

/// CT_TblPrBase order (§17.4.60).
const TBLPR_ORDER: &[&str] = &[
    "tblStyle", "tblpPr", "tblOverlap", "bidiVisual", "tblStyleRowBandSize", "tblStyleColBandSize",
    "tblW", "jc", "tblCellSpacing", "tblInd", "tblBorders", "shd", "tblLayout", "tblCellMar",
    "tblLook", "tblCaption", "tblDescription", "tblPrChange",
];

/// CT_TcPr order (§17.4.70).
const TCPR_ORDER: &[&str] = &[
    "cnfStyle", "tcW", "gridSpan", "hMerge", "vMerge", "tcBorders", "shd", "noWrap", "tcMar",
    "textDirection", "tcFitText", "vAlign", "hideMark", "headers", "cellIns", "cellDel",
    "cellMerge", "tcPrChange",
];

/// Local name of the root element of a raw XML fragment (`<w:foo …>` → `foo`).
fn raw_local_name(xml: &str) -> String {
    let t = xml.trim_start();
    let t = t.strip_prefix('<').unwrap_or(t);
    let end = t.find(|c: char| c.is_whitespace() || c == '>' || c == '/').unwrap_or(t.len());
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
        b.color.map_or("auto".to_owned(), |c| c.to_hex())
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
        v.push(("pStyle".into(), format!("<w:pStyle w:val=\"{}\"/>", escape(s))));
    }
    v.extend(flag("keepNext", p.keep_next));
    v.extend(flag("keepLines", p.keep_lines));
    v.extend(flag("pageBreakBefore", p.page_break_before));
    v.extend(flag("widowControl", p.widow_control));
    if let Some(n) = p.num {
        v.push((
            "numPr".into(),
            format!("<w:numPr><w:ilvl w:val=\"{}\"/><w:numId w:val=\"{}\"/></w:numPr>", n.ilvl, n.num_id),
        ));
    }
    if let Some(b) = &p.borders {
        v.push(("pBdr".into(), borders_xml("pBdr", b, true)));
    }
    if let Some(c) = p.shading {
        v.push(("shd".into(), format!("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>", c.to_hex())));
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
    if let Some(a) = p.align {
        v.push(("jc".into(), format!("<w:jc w:val=\"{}\"/>", jc(a))));
    }
    if let Some(l) = p.outline_level {
        v.push(("outlineLvl".into(), format!("<w:outlineLvl w:val=\"{l}\"/>")));
    }
    v.extend(raw_items(&p.unknown));
    v
}

fn rpr_children(p: &RunProps) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    if let Some(s) = &p.style {
        v.push(("rStyle".into(), format!("<w:rStyle w:val=\"{}\"/>", escape(s))));
    }
    if p.font.is_some() || p.font_east_asia.is_some() || p.font_cs.is_some() {
        let mut s = String::from("<w:rFonts");
        if let Some(f) = &p.font {
            let f = escape(f);
            let _ = write!(s, " w:ascii=\"{f}\" w:hAnsi=\"{f}\"");
        }
        if let Some(f) = &p.font_east_asia {
            let _ = write!(s, " w:eastAsia=\"{}\"", escape(f));
        }
        if let Some(f) = &p.font_cs {
            let _ = write!(s, " w:cs=\"{}\"", escape(f));
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
        v.push(("color".into(), format!("<w:color w:val=\"{}\"/>", c.to_hex())));
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
        v.push(("highlight".into(), format!("<w:highlight w:val=\"{}\"/>", h.highlight_name())));
    }
    if let Some(u) = p.underline {
        let val = match u {
            Underline::None => "none",
            Underline::Single => "single",
            Underline::Double => "double",
            Underline::Thick => "thick",
            Underline::Dotted => "dotted",
            Underline::Dash => "dash",
            Underline::Wave => "wave",
            Underline::Words => "words",
        };
        v.push(("u".into(), format!("<w:u w:val=\"{val}\"/>")));
    }
    if let Some(c) = p.shading {
        v.push(("shd".into(), format!("<w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{}\"/>", c.to_hex())));
    }
    if let Some(va) = p.vert_align {
        let val = match va {
            VertAlign::Baseline => "baseline",
            VertAlign::Superscript => "superscript",
            VertAlign::Subscript => "subscript",
        };
        v.push(("vertAlign".into(), format!("<w:vertAlign w:val=\"{val}\"/>")));
    }
    v.extend(raw_items(&p.unknown));
    v
}

fn tblpr_children(t: &TableProps) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = Vec::new();
    if let Some(s) = &t.style {
        v.push(("tblStyle".into(), format!("<w:tblStyle w:val=\"{}\"/>", escape(s))));
    }
    if let Some(w) = t.width {
        v.push(("tblW".into(), width_xml("tblW", w)));
    }
    if let Some(a) = t.align {
        v.push(("jc".into(), format!("<w:jc w:val=\"{}\"/>", jc(a))));
    }
    if let Some(i) = t.indent {
        v.push(("tblInd".into(), format!("<w:tblInd w:w=\"{}\" w:type=\"dxa\"/>", i.0)));
    }
    if let Some(b) = &t.borders {
        v.push(("tblBorders".into(), borders_xml("tblBorders", b, false)));
    }
    if t.fixed_layout {
        v.push(("tblLayout".into(), "<w:tblLayout w:type=\"fixed\"/>".into()));
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
                            num: Some(NumRef { num_id: list, ilvl: 0 }),
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
                            underline: Some(Underline::Single),
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
                for v in [&mut set.default, &mut set.first, &mut set.even].into_iter().flatten() {
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
            assert_eq!(b.part(&p.name).map(|x| &x.data), Some(&p.data), "part {} changed", p.name);
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
        let Block::Paragraph(p) = &mut doc.sections[0].blocks[0] else { panic!() };
        p.runs[0].content = vec![RunContent::Text("ĐÃ SỬA".into())];
        let out = write_docx(&doc).unwrap();
        let pkg = Package::read(&out, &Limits::default()).unwrap();
        let xml = String::from_utf8(pkg.part("/word/document.xml").unwrap().data.clone()).unwrap();
        assert!(xml.contains("ĐÃ SỬA"));
        assert!(xml.contains(orig_second.as_ref()));
        assert_eq!(read_docx(&out).unwrap().plain_text().lines().next(), Some("ĐÃ SỬA"));
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
        assert_eq!(raw_local_name("  <w14:textOutline w14:w=\"1\">"), "textOutline");
    }
}
