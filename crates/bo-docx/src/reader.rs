//! `.docx` → `bo_model::Document` (ECMA-376 Part 1, §17 WordprocessingML).
//!
//! Every block and property container records its source XML so the writer can
//! reproduce unchanged content byte-for-byte (ADR-0005).

use crate::{DocxError, Result};
use bo_model::slide::Theme;

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const W14_NS: &str = "http://schemas.microsoft.com/office/word/2010/wordml";
const W15_NS: &str = "http://schemas.microsoft.com/office/word/2012/wordml";
use bo_model::{
    fingerprint, AbstractNum, Alignment, Anchor, Block, Border, Borders, Cell, CellMerge,
    CellProps, CellPropsChange, CellVAlign, Color, Columns, Comment, ContentControl, ControlKind,
    Document, Emu, GridChange, HalfPoints, HeaderFooter, HeightRule, Image, Lang, Level,
    LineSpacing, NotePos, NoteProps, NoteRestart, Num, NumFormat, NumRef, Numbering, PageBorder,
    PageBorderDisplay, PageBorders, ParaProps, ParaPropsChange, Paragraph, Raw, RevMark,
    RevisionInfo, Row, RowPropsChange, Run, RunContent, RunProps, RunPropsChange, Section,
    SectionBreak, SourcePackage, Style, StyleKind, StyleSheet, TabAlign, TabLeader, TabStop, Table,
    TableFloat, TableProps, TablePropsChange, TableWidth, TextDirection, ThemeFonts, Twips,
    Underline, VMerge, VertAlign, Wrap, WrapText,
};
use bo_opc::xml::{attr, unescape, XmlReader};
use bo_opc::{Limits, Package, Relationship, TargetMode, REL_OFFICE_DOCUMENT, REL_STYLES};
use quick_xml::events::{BytesStart, Event};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

/// ECMA-376 Part 1, §14.2.7 theme part relationship.
const REL_THEME: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme";
const REL_NUMBERING: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering";
const REL_SETTINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";
const REL_FOOTNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/footnotes";
const REL_ENDNOTES: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/endnotes";
pub(crate) const REL_COMMENTS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments";
/// [MS-DOCX] §2.2.x commentsExtended part relationship.
pub(crate) const REL_COMMENTS_EXT: &str =
    "http://schemas.microsoft.com/office/2011/relationships/commentsExtended";
/// [MS-DOCX] commentsIds part relationship.
pub(crate) const REL_COMMENTS_IDS: &str =
    "http://schemas.microsoft.com/office/2016/09/relationships/commentsIds";

/// Parsing context for one source part (relationships are part-relative).
struct Ctx<'p> {
    pkg: &'p Package,
    part: String,
    rels: HashMap<String, Relationship>,
    /// Document theme (colours of shapes, theme fonts).
    theme: Option<Arc<Theme>>,
}

impl<'p> Ctx<'p> {
    fn new(pkg: &'p Package, part: &str) -> Result<Self> {
        let rels = pkg
            .relationships(part)?
            .into_iter()
            .map(|r| (r.id.clone(), r))
            .collect();
        Ok(Ctx {
            pkg,
            part: part.to_owned(),
            rels,
            theme: None,
        })
    }

    /// Absolute part name of an internal relationship target.
    fn target(&self, id: &str) -> Option<String> {
        let r = self.rels.get(id)?;
        if r.mode == TargetMode::External {
            return None;
        }
        Some(bo_opc::resolve_target(&self.part, &r.target))
    }

    fn external(&self, id: &str) -> Option<String> {
        self.rels.get(id).map(|r| r.target.clone())
    }
}

/// Read a `.docx` file into the document model.
pub fn read_docx(bytes: &[u8]) -> Result<Document> {
    let pkg = Package::read(bytes, &Limits::default())?;
    let main = pkg
        .related_part("/", REL_OFFICE_DOCUMENT)?
        .ok_or(DocxError::NoMainDocument)?;
    let main_part = pkg.part(&main).ok_or(DocxError::NoMainDocument)?;
    let mut ctx = Ctx::new(&pkg, &main)?;
    let mut doc = Document::default();
    ctx.theme = read_theme(&pkg, &main, &mut doc);
    if let Some(p) = pkg
        .related_part(&main, REL_STYLES)?
        .and_then(|n| pkg.part(&n))
    {
        read_styles(&p.data, &p.name, &mut doc)?;
    }
    if let Some(p) = pkg
        .related_part(&main, REL_NUMBERING)?
        .and_then(|n| pkg.part(&n))
    {
        doc.numbering = read_numbering(&p.data, &p.name)?;
    }
    if let Some(p) = pkg
        .related_part(&main, REL_SETTINGS)?
        .and_then(|n| pkg.part(&n))
    {
        read_settings(&p.data, &p.name, &mut doc)?;
    }
    let mut media = BTreeMap::new();
    for (rel, notes) in [(REL_FOOTNOTES, true), (REL_ENDNOTES, false)] {
        if let Some(name) = pkg.related_part(&main, rel)? {
            if let Some(p) = pkg.part(&name) {
                let mut nctx = Ctx::new(&pkg, &name)?;
                nctx.theme.clone_from(&ctx.theme);
                let (map, notice) = read_notes(&p.data, &nctx, &mut media)?;
                if notes {
                    doc.footnotes = map;
                    doc.continuation_notice = notice.filter(|b| {
                        b.iter()
                            .any(|b| b.as_paragraph().is_none_or(|p| !p.plain_text().is_empty()))
                    });
                } else {
                    doc.endnotes = map;
                }
            }
        }
    }
    let mut comments_root = None;
    let mut comments_ids = false;
    if let Some(name) = pkg.related_part(&main, REL_COMMENTS)? {
        if let Some(p) = pkg.part(&name) {
            let mut cctx = Ctx::new(&pkg, &name)?;
            cctx.theme.clone_from(&ctx.theme);
            let (root, list) = read_comments(&p.data, &cctx, &mut media)?;
            comments_root = Some(root);
            doc.comments = list;
        }
    }
    if let Some(p) = pkg
        .related_part(&main, REL_COMMENTS_EXT)?
        .and_then(|n| pkg.part(&n))
    {
        read_comments_ext(&p.data, &p.name, &mut doc.comments)?;
    }
    if let Some(p) = pkg
        .related_part(&main, REL_COMMENTS_IDS)?
        .and_then(|n| pkg.part(&n))
    {
        comments_ids = true;
        read_comments_ids(&p.data, &p.name, &mut doc.comments)?;
    }
    let (prefix, suffix) = read_body(&main_part.data, &ctx, &mut doc, &mut media)?;
    doc.media = media;
    doc.source = Some(Arc::new(SourcePackage {
        bytes: Arc::new(bytes.to_vec()),
        main_part: main,
        body_prefix: prefix,
        body_suffix: suffix,
        styles_fingerprint: styles_fingerprint(&doc),
        numbering_fingerprint: fingerprint(&doc.numbering),
        comments_fingerprint: fingerprint(&doc.comments),
        comments_root,
        comments_ids,
        notes_fingerprint: crate::writer::notes_fingerprint(&doc),
        even_and_odd_headers: doc.even_and_odd_headers,
        background: doc.design.background,
        display_background_shape: doc.design.display_background_shape,
        protection: doc.protection.clone(),
    }));
    Ok(doc)
}

/// `w14:paraId` of a paragraph's start tag ([MS-DOCX] §2.6.x paraId).
pub(crate) fn para_id_of(p: &Paragraph) -> Option<String> {
    let xml = &p.src.as_ref()?.xml;
    let tag = &xml[..xml.find('>')?];
    let i = tag.find(":paraId=\"")? + ":paraId=\"".len();
    let rest = &tag[i..];
    Some(rest[..rest.find('"')?].to_owned())
}

/// ECMA-376 Part 1, §17.13.4.6 comments. Returns the root start tag and the comments.
fn read_comments(
    data: &[u8],
    ctx: &Ctx<'_>,
    media: &mut BTreeMap<String, Arc<Vec<u8>>>,
) -> Result<(String, Vec<Comment>)> {
    let mut r = XmlReader::new(data, &ctx.part);
    let mut out = Vec::new();
    let mut root = String::new();
    loop {
        match r.read()? {
            Event::Start(e) => match name(&e).as_slice() {
                // §17.13.4.2 comment
                b"comment" => {
                    let mut c = Comment {
                        id: int(&e, b"id").unwrap_or(0) as i32,
                        author: attr(&e, b"author").unwrap_or_default(),
                        initials: attr(&e, b"initials"),
                        date: attr(&e, b"date"),
                        ..Comment::default()
                    };
                    c.blocks = read_blocks(&mut r, ctx, media, &mut None)?;
                    c.para_id = c
                        .blocks
                        .iter()
                        .rev()
                        .find_map(Block::as_paragraph)
                        .and_then(para_id_of);
                    out.push(c);
                }
                b"comments" => root = r.slice(0, r.pos()),
                _ => r.skip_element()?,
            },
            Event::Empty(e) if name(&e) == b"comments" => {
                root = r.slice(0, r.pos());
                root.truncate(root.trim_end().trim_end_matches("/>").len());
                root.push('>');
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok((root, out))
}

/// [MS-DOCX] §2.5.2 `w15:commentsEx`: threading (`paraIdParent`) and the done flag.
fn read_comments_ext(data: &[u8], part: &str, comments: &mut [Comment]) -> Result<()> {
    let mut r = XmlReader::new(data, part);
    loop {
        match r.read()? {
            Event::Start(e) | Event::Empty(e) if name(&e) == b"commentEx" => {
                let Some(pid) = attr(&e, b"paraId") else {
                    continue;
                };
                if let Some(c) = comments
                    .iter_mut()
                    .find(|c| c.para_id.as_deref() == Some(pid.as_str()))
                {
                    c.parent_para_id = attr(&e, b"paraIdParent");
                    c.done = matches!(attr(&e, b"done").as_deref(), Some("1" | "true"));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(())
}

/// [MS-DOCX] `w16cid:commentsIds`: durable comment ids.
fn read_comments_ids(data: &[u8], part: &str, comments: &mut [Comment]) -> Result<()> {
    let mut r = XmlReader::new(data, part);
    loop {
        match r.read()? {
            Event::Start(e) | Event::Empty(e) if name(&e) == b"commentId" => {
                let Some(pid) = attr(&e, b"paraId") else {
                    continue;
                };
                if let Some(c) = comments
                    .iter_mut()
                    .find(|c| c.para_id.as_deref() == Some(pid.as_str()))
                {
                    c.durable_id = attr(&e, b"durableId");
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(())
}

/// Fingerprint of everything written to `styles.xml`.
pub(crate) fn styles_fingerprint(doc: &Document) -> u64 {
    fingerprint(&(&doc.styles, &doc.default_run, &doc.default_para))
}

/// Visit the children of the element whose `Start` was just read. The callback
/// receives the child, whether it is a `Start` (must be consumed or return
/// `false` to have it skipped) and its start offset.
fn children<'a, F>(r: &mut XmlReader<'a>, mut f: F) -> Result<()>
where
    F: FnMut(&mut XmlReader<'a>, &BytesStart<'a>, bool, usize) -> Result<bool>,
{
    let end = r.depth() - 1;
    loop {
        let before = r.pos();
        match r.read()? {
            Event::Start(e) => {
                if !f(r, &e, true, before)? {
                    r.skip_element()?;
                }
            }
            Event::Empty(e) => {
                f(r, &e, false, before)?;
            }
            Event::End(_) if r.depth() == end => return Ok(()),
            Event::Eof => return Err(r.err("unexpected end of document").into()),
            _ => {}
        }
    }
}

/// Consume (if needed) and capture the current child as raw XML.
fn capture(r: &mut XmlReader<'_>, is_start: bool, before: usize) -> Result<Raw> {
    if is_start {
        r.skip_element()?;
    }
    Ok(Raw::new(r.slice(before, r.pos()), 0))
}

fn name(e: &BytesStart<'_>) -> Vec<u8> {
    e.local_name().as_ref().as_bytes().to_vec()
}

fn on_off(e: &BytesStart<'_>) -> bool {
    // §17.17.4 ST_OnOff: absent val means true.
    !matches!(attr(e, b"val").as_deref(), Some("0" | "false" | "off"))
}

/// An `ST_OnOff` attribute value (§17.17.4).
fn on_off_value(v: &str) -> bool {
    !matches!(v, "0" | "false" | "off")
}

fn twips(e: &BytesStart<'_>, a: &[u8]) -> Option<Twips> {
    attr(e, a)
        .and_then(|v| v.parse::<f64>().ok())
        .map(|v| Twips(v as i32))
}

fn int(e: &BytesStart<'_>, a: &[u8]) -> Option<i64> {
    attr(e, a)
        .and_then(|v| v.trim().parse::<f64>().ok())
        .map(|v| v as i64)
}

fn color_attr(e: &BytesStart<'_>, a: &[u8]) -> Option<Color> {
    attr(e, a).as_deref().and_then(Color::from_hex)
}

fn align(v: Option<&str>) -> Option<Alignment> {
    match v? {
        "center" => Some(Alignment::Center),
        "right" | "end" => Some(Alignment::Right),
        "both" | "distribute" | "lowKashida" | "mediumKashida" | "highKashida"
        | "thaiDistribute" => Some(Alignment::Justify),
        "left" | "start" => Some(Alignment::Left),
        _ => None,
    }
}

// ---------------------------------------------------------------- styles

/// ECMA-376 Part 1, §17.7 Styles.
fn read_styles(data: &[u8], part: &str, doc: &mut Document) -> Result<()> {
    let mut r = XmlReader::new(data, part);
    let mut sheet = StyleSheet::default();
    loop {
        let before = r.pos();
        match r.read()? {
            Event::Start(e) => match name(&e).as_slice() {
                // §17.7.5.1 docDefaults
                b"docDefaults" => children(&mut r, |r, e, s, _| {
                    if !s {
                        return Ok(true);
                    }
                    match name(e).as_slice() {
                        b"rPrDefault" => {
                            children(r, |r, e, s, _| {
                                if s && name(e) == b"rPr" {
                                    doc.default_run = read_rpr(r)?;
                                    return Ok(true);
                                }
                                Ok(!s)
                            })?;
                            Ok(true)
                        }
                        b"pPrDefault" => {
                            children(r, |r, e, s, _| {
                                if s && name(e) == b"pPr" {
                                    doc.default_para = read_ppr(r, None)?;
                                    return Ok(true);
                                }
                                Ok(!s)
                            })?;
                            Ok(true)
                        }
                        _ => Ok(false),
                    }
                })?,
                // §17.7.4.5 latentStyles
                b"latentStyles" => {
                    sheet.latent = Some(capture(&mut r, true, before)?);
                }
                // §17.7.4.17 style
                b"style" => {
                    let st = read_style(&mut r, &e, before)?;
                    sheet.styles.push(st);
                }
                b"styles" => {}
                _ => r.skip_element()?,
            },
            Event::Eof => break,
            _ => {}
        }
    }
    doc.styles = sheet;
    Ok(())
}

fn read_style<'a>(r: &mut XmlReader<'a>, e: &BytesStart<'a>, before: usize) -> Result<Style> {
    let kind = match attr(e, b"type").as_deref() {
        Some("character") => StyleKind::Character,
        Some("table") => StyleKind::Table,
        Some("numbering") => StyleKind::Numbering,
        _ => StyleKind::Paragraph,
    };
    let id = attr(e, b"styleId").unwrap_or_default();
    let mut st = Style::new(&id, &id, kind);
    st.quick = false;
    st.is_default = attr(e, b"default").is_some_and(|v| v == "1" || v == "true");
    children(r, |r, e, s, _| {
        match name(e).as_slice() {
            b"name" => st.name = attr(e, b"val").unwrap_or_default(),
            b"basedOn" => st.based_on = attr(e, b"val"),
            b"next" => st.next = attr(e, b"val"),
            b"link" => st.linked = attr(e, b"val"),
            b"qFormat" => st.quick = on_off(e),
            b"pPr" if s => {
                st.ppr = read_ppr(r, None)?;
                return Ok(true);
            }
            b"rPr" if s => {
                st.rpr = read_rpr(r)?;
                return Ok(true);
            }
            b"tblPr" if s => {
                st.tblpr = Some(read_tblpr(r)?);
                return Ok(true);
            }
            _ => return Ok(!s),
        }
        Ok(!s)
    })?;
    st.src = Some(Raw::new(r.slice(before, r.pos()), 0));
    Ok(st)
}

// ---------------------------------------------------------------- numbering

/// ECMA-376 Part 1, §17.9 Numbering.
fn read_numbering(data: &[u8], part: &str) -> Result<Numbering> {
    let mut r = XmlReader::new(data, part);
    let mut n = Numbering::default();
    loop {
        match r.read()? {
            Event::Start(e) => match name(&e).as_slice() {
                // §17.9.1 abstractNum
                b"abstractNum" => {
                    let id = int(&e, b"abstractNumId").unwrap_or(0) as i32;
                    let mut levels = Vec::new();
                    children(&mut r, |r, e, s, _| {
                        if s && name(e) == b"lvl" {
                            levels.push(read_level(r, e)?);
                            return Ok(true);
                        }
                        Ok(!s)
                    })?;
                    n.abstracts.push(AbstractNum { id, levels });
                }
                // §17.9.15 num
                b"num" => {
                    let id = int(&e, b"numId").unwrap_or(0) as i32;
                    let mut abstract_id = 0;
                    let mut overrides = Vec::new();
                    children(&mut r, |r, e, s, _| {
                        match name(e).as_slice() {
                            b"abstractNumId" => abstract_id = int(e, b"val").unwrap_or(0) as i32,
                            b"lvlOverride" if s => {
                                let ilvl = int(e, b"ilvl").unwrap_or(0) as u8;
                                children(r, |_, e, s, _| {
                                    if name(e) == b"startOverride" {
                                        overrides.push((ilvl, int(e, b"val").unwrap_or(1) as i32));
                                    }
                                    Ok(!s)
                                })?;
                                return Ok(true);
                            }
                            _ => {}
                        }
                        Ok(!s)
                    })?;
                    n.nums.push(Num {
                        id,
                        abstract_id,
                        start_overrides: overrides,
                    });
                }
                b"numbering" => {}
                _ => r.skip_element()?,
            },
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(n)
}

/// §17.9.6 lvl
fn read_level<'a>(r: &mut XmlReader<'a>, e: &BytesStart<'a>) -> Result<Level> {
    let mut l = Level {
        ilvl: int(e, b"ilvl").unwrap_or(0).clamp(0, 8) as u8,
        start: 1,
        format: NumFormat::Decimal,
        text: String::new(),
        align: Alignment::Left,
        ppr: ParaProps::default(),
        rpr: RunProps::default(),
        restart: None,
        suffix: "tab".into(),
        pstyle: None,
    };
    children(r, |r, e, s, _| {
        match name(e).as_slice() {
            b"start" => l.start = int(e, b"val").unwrap_or(1) as i32,
            b"numFmt" => l.format = NumFormat::parse(&attr(e, b"val").unwrap_or_default()),
            b"lvlText" => l.text = attr(e, b"val").unwrap_or_default(),
            b"lvlJc" => l.align = align(attr(e, b"val").as_deref()).unwrap_or_default(),
            b"lvlRestart" => l.restart = int(e, b"val").map(|v| v.clamp(0, 9) as u8),
            // §17.9.23 pStyle: paragraph style linked to the level.
            b"pStyle" => l.pstyle = attr(e, b"val").filter(|v| !v.is_empty()),
            b"suff" => l.suffix = attr(e, b"val").unwrap_or_else(|| "tab".into()),
            b"pPr" if s => {
                l.ppr = read_ppr(r, None)?;
                return Ok(true);
            }
            b"rPr" if s => {
                l.rpr = read_rpr(r)?;
                return Ok(true);
            }
            _ => {}
        }
        Ok(!s)
    })?;
    Ok(l)
}

/// §17.15.1 settings — defaultTabStop, evenAndOddHeaders.
fn read_settings(data: &[u8], part: &str, doc: &mut Document) -> Result<()> {
    let mut r = XmlReader::new(data, part);
    loop {
        match r.read()? {
            // §17.11.11 footnotePr / §17.11.4 endnotePr (document-wide, CT_FtnDocProps /
            // CT_EdnDocProps: the separator references they hold are not needed).
            Event::Start(e) if matches!(name(&e).as_slice(), b"footnotePr" | b"endnotePr") => {
                let np = read_note_pr(&mut r)?;
                if name(&e) == b"footnotePr" {
                    doc.footnote_pr = np;
                } else {
                    doc.endnote_pr = np;
                }
            }
            Event::Start(e) | Event::Empty(e) => match name(&e).as_slice() {
                b"defaultTabStop" => doc.default_tab_stop = twips(&e, b"val"),
                b"evenAndOddHeaders" => doc.even_and_odd_headers = on_off(&e),
                b"autoHyphenation" => doc.auto_hyphenation = on_off(&e), // §17.15.1.10
                // §17.15.1.32 displayBackgroundShape
                b"displayBackgroundShape" => doc.design.display_background_shape = on_off(&e),
                // §17.15.1.29 documentProtection
                b"documentProtection" => {
                    doc.protection = Some(crate::forms::read_protection(&e));
                }
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(())
}

/// Notes of a part by id and the continuation notice (§17.18.33).
type NotesPart = (BTreeMap<i32, Vec<Block>>, Option<Vec<Block>>);

/// §17.11 footnotes / endnotes.
fn read_notes(
    data: &[u8],
    ctx: &Ctx<'_>,
    media: &mut BTreeMap<String, Arc<Vec<u8>>>,
) -> Result<NotesPart> {
    let mut r = XmlReader::new(data, &ctx.part);
    let mut out = BTreeMap::new();
    let mut notice = None;
    loop {
        match r.read()? {
            Event::Start(e) => match name(&e).as_slice() {
                b"footnote" | b"endnote" => {
                    let id = int(&e, b"id").unwrap_or(0) as i32;
                    // §17.18.33 ST_FtnEdn: separator, continuationSeparator,
                    // continuationNotice or normal.
                    let kind = attr(&e, b"type");
                    let blocks = read_blocks(&mut r, ctx, media, &mut None)?;
                    match kind.as_deref() {
                        None | Some("normal") => {
                            out.insert(id, blocks);
                        }
                        Some("continuationNotice") => notice = Some(blocks),
                        _ => {}
                    }
                }
                b"footnotes" | b"endnotes" => {}
                _ => r.skip_element()?,
            },
            Event::Eof => break,
            _ => {}
        }
    }
    Ok((out, notice))
}

// ---------------------------------------------------------------- body

/// ECMA-376 Part 1, §17.2.2 body. Returns the text before / after the body children.
fn read_body(
    data: &[u8],
    ctx: &Ctx<'_>,
    doc: &mut Document,
    media: &mut BTreeMap<String, Arc<Vec<u8>>>,
) -> Result<(String, String)> {
    let mut r = XmlReader::new(data, &ctx.part);
    loop {
        match r.read()? {
            // §17.2.1 background (before the body): the page colour.
            Event::Start(e) | Event::Empty(e) if name(&e) == b"background" => {
                doc.design.background = color_attr(&e, b"color");
            }
            Event::Start(e) if name(&e) == b"body" => {
                let prefix = r.slice(0, r.pos());
                let mut sections: Vec<Section> = Vec::new();
                let mut blocks: Vec<Block> = Vec::new();
                let end = r.depth() - 1;
                let body_end;
                loop {
                    let before = r.pos();
                    match r.read()? {
                        Event::Start(e) => {
                            let n = name(&e);
                            if n == b"sectPr" {
                                let mut sec = read_sect_pr(&mut r, ctx, media, before)?;
                                sec.blocks = std::mem::take(&mut blocks);
                                sections.push(sec);
                            } else {
                                let mut sect = None;
                                let b = read_block(&mut r, &e, ctx, media, before, &mut sect)?;
                                blocks.extend(b);
                                if let Some(mut sec) = sect {
                                    sec.blocks = std::mem::take(&mut blocks);
                                    sections.push(sec);
                                }
                            }
                        }
                        Event::Empty(e) => {
                            if name(&e) == b"sectPr" {
                                let mut sec = Section::default();
                                sec.src = Some(Raw::new(
                                    r.slice(before, r.pos()),
                                    sec.props_fingerprint(),
                                ));
                                sec.blocks = std::mem::take(&mut blocks);
                                sections.push(sec);
                            } else {
                                blocks.push(Block::Opaque(Raw::new(r.slice(before, r.pos()), 0)));
                            }
                        }
                        Event::End(_) if r.depth() == end => {
                            body_end = before;
                            break;
                        }
                        Event::Eof => return Err(r.err("unexpected end of body").into()),
                        _ => {}
                    }
                }
                if !blocks.is_empty() || sections.is_empty() {
                    // Body without a final sectPr: defaults (§17.6.17).
                    sections.push(Section::new(blocks));
                }
                doc.sections = sections;
                let suffix = r.slice(body_end, data.len());
                return Ok((prefix, suffix));
            }
            Event::Eof => {
                doc.sections = vec![Section::default()];
                return Ok((String::new(), String::new()));
            }
            _ => {}
        }
    }
}

/// Read blocks until the end of the current element.
fn read_blocks(
    r: &mut XmlReader<'_>,
    ctx: &Ctx<'_>,
    media: &mut BTreeMap<String, Arc<Vec<u8>>>,
    sect: &mut Option<Section>,
) -> Result<Vec<Block>> {
    let mut out = Vec::new();
    let end = r.depth() - 1;
    loop {
        let before = r.pos();
        match r.read()? {
            Event::Start(e) => {
                if name(&e) == b"tcPr" {
                    // handled by caller (table cells); never reached here
                    r.skip_element()?;
                } else {
                    let b = read_block(r, &e, ctx, media, before, sect)?;
                    out.extend(b);
                }
            }
            Event::Empty(e) => {
                let raw = Raw::new(r.slice(before, r.pos()), 0);
                if name(&e) == b"p" {
                    let mut p = Paragraph::default();
                    p.src = Some(Raw::new(raw.xml, p.content_fingerprint()));
                    out.push(Block::Paragraph(p));
                } else {
                    out.push(Block::Opaque(raw));
                }
            }
            Event::End(_) if r.depth() == end => return Ok(out),
            Event::Eof => return Err(r.err("unexpected end").into()),
            _ => {}
        }
    }
}

fn read_block<'a>(
    r: &mut XmlReader<'a>,
    e: &BytesStart<'a>,
    ctx: &Ctx<'_>,
    media: &mut BTreeMap<String, Arc<Vec<u8>>>,
    before: usize,
    sect: &mut Option<Section>,
) -> Result<Vec<Block>> {
    if name(e) == b"sdt" {
        return read_sdt_block(r, ctx, media, before, sect);
    }
    Ok(vec![match name(e).as_slice() {
        b"p" => {
            let mut p = read_paragraph(r, ctx, media, sect)?;
            p.src = Some(Raw::new(r.slice(before, r.pos()), p.content_fingerprint()));
            Block::Paragraph(p)
        }
        b"tbl" => {
            let mut t = read_table(r, ctx, media)?;
            t.src = Some(Raw::new(r.slice(before, r.pos()), t.content_fingerprint()));
            Block::Table(t)
        }
        _ => Block::Opaque(capture(r, true, before)?),
    }])
}

/// §17.5.2.29 sdt around block content: its paragraphs with markers at the start of the first and
/// the end of the last; a control around tables or nothing stays as read.
fn read_sdt_block(
    r: &mut XmlReader<'_>,
    ctx: &Ctx<'_>,
    media: &mut BTreeMap<String, Arc<Vec<u8>>>,
    before: usize,
    sect: &mut Option<Section>,
) -> Result<Vec<Block>> {
    let mut cc = None;
    let mut inner: Option<Vec<Block>> = None;
    children(r, |r, e, s, at| match name(e).as_slice() {
        b"sdtPr" => {
            let raw = capture(r, s, at)?;
            cc = Some(parse_sdt_pr(&raw.xml, true));
            Ok(true)
        }
        b"sdtContent" if s => {
            inner = Some(read_blocks(r, ctx, media, sect)?);
            Ok(true)
        }
        _ => Ok(!s),
    })?;
    let raw = Raw::new(r.slice(before, r.pos()), 0);
    match (cc, inner) {
        (Some(cc), Some(mut blocks))
            if !blocks.is_empty() && blocks.iter().all(|b| matches!(b, Block::Paragraph(_))) =>
        {
            let n = blocks.len();
            for (k, b) in blocks.iter_mut().enumerate() {
                let Block::Paragraph(p) = b else { continue };
                if k == 0 {
                    p.runs.insert(
                        0,
                        Run {
                            props: RunProps::default(),
                            content: vec![RunContent::ControlStart(Box::new(cc.clone()))],
                        },
                    );
                }
                if k + 1 == n {
                    p.runs.push(Run {
                        props: RunProps::default(),
                        content: vec![RunContent::ControlEnd(true)],
                    });
                }
                // The paragraph's own XML still stands for it (the markers are written around).
                let fp = p.content_fingerprint();
                if let Some(src) = p.src.take() {
                    p.src = Some(Raw::new(src.xml, fp));
                }
            }
            Ok(blocks)
        }
        _ => Ok(vec![Block::Opaque(raw)]),
    }
}

/// §17.5.2.38 sdtPr: id, alias, tag, lock, placeholder and the kind of control.
pub(crate) fn parse_sdt_pr(xml: &str, block: bool) -> ContentControl {
    let mut cc = ContentControl {
        id: None,
        alias: None,
        tag: None,
        kind: ControlKind::RichText,
        lock: None,
        placeholder: false,
        block,
        props_xml: Some(xml.to_owned()),
    };
    let wrapped =
        format!("<x xmlns:w=\"{W_NS}\" xmlns:w14=\"{W14_NS}\" xmlns:w15=\"{W15_NS}\">{xml}</x>");
    let Ok(root) = bo_drawingml::dom::parse(wrapped.as_bytes(), "/sdtPr") else {
        return cc;
    };
    let Some(pr) = root.elements().next() else {
        return cc;
    };
    let val = |e: &bo_model::slide::XmlElement| {
        e.attrs
            .iter()
            .find(|(k, _)| k.rsplit(':').next() == Some("val"))
            .map(|(_, v)| v.clone())
    };
    let attr_of = |e: &bo_model::slide::XmlElement, local: &str| {
        e.attrs
            .iter()
            .find(|(k, _)| k.rsplit(':').next() == Some(local))
            .map(|(_, v)| v.clone())
    };
    let on = |v: Option<String>| !matches!(v.as_deref(), Some("0" | "false" | "off"));
    let items = |e: &bo_model::slide::XmlElement| -> Vec<(String, String)> {
        e.elements()
            .filter(|li| li.local() == "listItem")
            .map(|li| {
                let v = attr_of(li, "value").unwrap_or_default();
                (attr_of(li, "displayText").unwrap_or_else(|| v.clone()), v)
            })
            .collect()
    };
    let symbol = |e: Option<&bo_model::slide::XmlElement>, default: char| {
        e.and_then(|s| attr_of(s, "val"))
            .and_then(|h| u32::from_str_radix(&h, 16).ok())
            .and_then(char::from_u32)
            .unwrap_or(default)
    };
    for c in pr.elements() {
        match c.local() {
            "id" => cc.id = val(c).and_then(|v| v.trim().parse().ok()),
            "alias" => cc.alias = val(c),
            "tag" => cc.tag = val(c),
            "lock" => cc.lock = val(c),
            "showingPlcHdr" => cc.placeholder = on(val(c)),
            "text" => {
                cc.kind = ControlKind::Text {
                    multi_line: attr_of(c, "multiLine").is_some_and(|v| v == "1" || v == "true"),
                };
            }
            "richText" => cc.kind = ControlKind::RichText,
            "picture" => cc.kind = ControlKind::Picture,
            "comboBox" => cc.kind = ControlKind::ComboBox(items(c)),
            "dropDownList" => cc.kind = ControlKind::DropDown(items(c)),
            "date" => {
                cc.kind = ControlKind::Date {
                    format: c
                        .elements()
                        .find(|x| x.local() == "dateFormat")
                        .and_then(val)
                        .unwrap_or_else(|| "dd/MM/yyyy".into()),
                    full_date: attr_of(c, "fullDate"),
                };
            }
            "checkbox" => {
                let el = |n: &str| c.elements().find(|x| x.local() == n);
                cc.kind = ControlKind::Checkbox {
                    checked: el("checked").is_some_and(|x| on(val(x))),
                    on: symbol(el("checkedState"), '\u{2612}'),
                    off: symbol(el("uncheckedState"), '\u{2610}'),
                };
            }
            "repeatingSection" => cc.kind = ControlKind::RepeatingSection,
            "group" => cc.kind = ControlKind::Group,
            "citation" | "bibliography" | "equation" | "docPartObj" | "docPartList" => {
                cc.kind = ControlKind::Other;
            }
            _ => {}
        }
    }
    cc
}

/// ECMA-376 Part 1, §17.3.1.22 p.
fn read_paragraph(
    r: &mut XmlReader<'_>,
    ctx: &Ctx<'_>,
    media: &mut BTreeMap<String, Arc<Vec<u8>>>,
    sect: &mut Option<Section>,
) -> Result<Paragraph> {
    let mut p = Paragraph::default();
    read_para_children(r, ctx, media, sect, &mut p, &RunCtx::default())?;
    crate::forms::collapse_form_fields(&mut p.runs);
    collapse_simple_fields(&mut p.runs);
    Ok(p)
}

/// Properties inherited by runs from their paragraph-level wrappers.
#[derive(Clone, Default)]
struct RunCtx {
    link: Option<String>,
    ins: Option<RevMark>,
    del: Option<RevMark>,
}

impl RunCtx {
    fn apply(&self, props: &mut RunProps) {
        if let Some(l) = &self.link {
            props.link = Some(l.clone());
        }
        if self.ins.is_some() {
            props.ins.clone_from(&self.ins);
        }
        if self.del.is_some() {
            props.del.clone_from(&self.del);
        }
    }
}

/// Attributes of a tracked change (ECMA-376 Part 1, §17.13.5 `CT_TrackChange`).
fn rev_info(e: &BytesStart<'_>) -> RevisionInfo {
    RevisionInfo {
        id: int(e, b"id").unwrap_or(0) as i32,
        author: attr(e, b"author").unwrap_or_default(),
        date: attr(e, b"date"),
    }
}

fn read_para_children(
    r: &mut XmlReader<'_>,
    ctx: &Ctx<'_>,
    media: &mut BTreeMap<String, Arc<Vec<u8>>>,
    sect: &mut Option<Section>,
    p: &mut Paragraph,
    rc: &RunCtx,
) -> Result<()> {
    children(r, |r, e, s, before| {
        match name(e).as_slice() {
            b"pPr" if s => {
                p.props = read_ppr(r, Some((ctx, &mut *media, &mut *sect)))?;
            }
            b"r" if s => {
                let mut run = read_run(r, ctx, media)?;
                rc.apply(&mut run.props);
                p.runs.push(run);
            }
            // §17.16.22 hyperlink
            b"hyperlink" if s => {
                let target = attr(e, b"id")
                    .and_then(|id| ctx.external(&id).or_else(|| ctx.target(&id)))
                    .or_else(|| attr(e, b"anchor").map(|a| format!("#{a}")));
                let inner = RunCtx {
                    link: target.or_else(|| rc.link.clone()),
                    ..rc.clone()
                };
                read_para_children(r, ctx, media, sect, p, &inner)?;
            }
            // §17.16.19 fldSimple
            b"fldSimple" => {
                let instr = attr(e, b"instr").unwrap_or_default();
                // §17.16.18 fldLock.
                let locked =
                    attr(e, b"fldLock").is_some_and(|v| v == "1" || v == "true" || v == "on");
                let mut inner = Paragraph::default();
                if s {
                    read_para_children(r, ctx, media, sect, &mut inner, rc)?;
                }
                let mut props = inner
                    .runs
                    .first()
                    .map(|x| x.props.clone())
                    .unwrap_or_default();
                rc.apply(&mut props);
                p.runs.push(Run {
                    props,
                    content: vec![RunContent::Field {
                        instr: instr.trim().to_owned(),
                        cached: inner.plain_text(),
                        locked,
                    }],
                });
            }
            // §17.13.5.18 ins / §17.13.5.25 moveTo: inserted content; §17.13.5.14 del /
            // §17.13.5.22 moveFrom: deleted content. Both are modelled on the runs.
            n @ (b"ins" | b"moveTo" | b"del" | b"moveFrom") if s => {
                let mark = RevMark {
                    info: rev_info(e),
                    moved: matches!(n, b"moveTo" | b"moveFrom"),
                };
                let mut inner = rc.clone();
                if matches!(n, b"ins" | b"moveTo") {
                    inner.ins = Some(mark);
                } else {
                    inner.del = Some(mark);
                }
                read_para_children(r, ctx, media, sect, p, &inner)?;
            }
            // §17.13.4.4 commentRangeStart / §17.13.4.3 commentRangeEnd
            n @ (b"commentRangeStart" | b"commentRangeEnd") => {
                if s {
                    r.skip_element()?;
                }
                let id = int(e, b"id").unwrap_or(0) as i32;
                p.runs.push(Run {
                    props: RunProps {
                        link: rc.link.clone(),
                        ..RunProps::default()
                    },
                    content: vec![if n == b"commentRangeStart" {
                        RunContent::CommentStart(id)
                    } else {
                        RunContent::CommentEnd(id)
                    }],
                });
            }
            // smartTag/customXml/sdt wrappers descend.
            b"smartTag" | b"customXml" if s => {
                read_para_children(r, ctx, media, sect, p, rc)?;
            }
            // §17.5.2.31 sdt (run-level content control): its content between markers.
            b"sdt" if s => {
                let mut started = false;
                children(r, |r, e, s, at| match name(e).as_slice() {
                    b"sdtPr" => {
                        let raw = capture(r, s, at)?;
                        p.runs.push(Run {
                            props: RunProps::default(),
                            content: vec![RunContent::ControlStart(Box::new(parse_sdt_pr(
                                &raw.xml, false,
                            )))],
                        });
                        started = true;
                        Ok(true)
                    }
                    b"sdtContent" if s => {
                        read_para_children(r, ctx, media, sect, p, rc)?;
                        Ok(true)
                    }
                    _ => Ok(!s),
                })?;
                if started {
                    p.runs.push(Run {
                        props: RunProps::default(),
                        content: vec![RunContent::ControlEnd(false)],
                    });
                }
            }
            // Other paragraph-level markup is kept verbatim.
            _ => {
                let raw = capture(r, s, before)?;
                p.runs.push(Run {
                    props: RunProps::default(),
                    content: vec![RunContent::Markup(raw)],
                });
            }
        }
        Ok(true)
    })
}

/// Collapse `begin … instr … separate … result … end` sequences of PAGE / NUMPAGES /
/// SECTIONPAGES / DATE / TIME fields that are complete inside one paragraph into a
/// single [`RunContent::Field`], so layout can compute the value.
fn collapse_simple_fields(runs: &mut Vec<Run>) {
    let is_fld = |c: &RunContent, kind: &str| match c {
        RunContent::Opaque(raw) => {
            raw.xml.contains("fldChar") && raw.xml.contains(&format!("\"{kind}\""))
        }
        _ => false,
    };
    let mut i = 0;
    while i < runs.len() {
        if !runs[i].content.iter().any(|c| is_fld(c, "begin")) {
            i += 1;
            continue;
        }
        // Find matching end (no nesting supported for collapse).
        let mut instr = String::new();
        let mut cached = String::new();
        let mut in_result = false;
        let mut end = None;
        let mut nested = false;
        // Only field structure and result text inside the range (nothing would be lost).
        let mut clean = true;
        for (j, run) in runs.iter().enumerate().skip(i) {
            for c in &run.content {
                match c {
                    c if is_fld(c, "begin") && j != i => nested = true,
                    c if is_fld(c, "begin") => {}
                    c if is_fld(c, "separate") => in_result = true,
                    c if is_fld(c, "end") => end = Some(j),
                    RunContent::Opaque(raw) if raw.xml.contains("instrText") => {
                        instr.push_str(&instr_text(&raw.xml));
                    }
                    RunContent::Text(t) if in_result => cached.push_str(t),
                    _ => clean = false,
                }
                if end.is_some() {
                    break;
                }
            }
            if end.is_some() {
                break;
            }
        }
        let kind = instr
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_uppercase();
        // Page fields are computed by layout; SEQ (§17.16.5.56), REF (§17.16.5.49),
        // PAGEREF (§17.16.5.42) and MERGEFIELD (§17.16.5.35) by "update fields" and
        // mail merge — those only when the range holds nothing else.
        let simple = matches!(
            kind.as_str(),
            "PAGE" | "NUMPAGES" | "SECTIONPAGES" | "DATE" | "TIME"
        ) || (clean
            && matches!(kind.as_str(), "SEQ" | "REF" | "PAGEREF" | "MERGEFIELD"));
        match end {
            Some(j) if simple && !nested => {
                let props = runs[i].props.clone();
                runs.splice(
                    i..=j,
                    [Run {
                        props,
                        content: vec![RunContent::Field {
                            instr: instr.trim().to_owned(),
                            cached,
                            locked: false,
                        }],
                    }],
                );
                i += 1;
            }
            _ => i += 1,
        }
    }
}

/// Extract the text of a `w:instrText` element.
fn instr_text(xml: &str) -> String {
    let Some(gt) = xml.find('>') else {
        return String::new();
    };
    let rest = &xml[gt + 1..];
    let end = rest.rfind("</").unwrap_or(rest.len());
    unescape(&rest[..end])
}

/// ECMA-376 Part 1, §17.3.2.25 r.
fn read_run(
    r: &mut XmlReader<'_>,
    ctx: &Ctx<'_>,
    media: &mut BTreeMap<String, Arc<Vec<u8>>>,
) -> Result<Run> {
    let mut run = Run::default();
    children(r, |r, e, s, before| {
        match name(e).as_slice() {
            b"rPr" if s => run.props = read_rpr(r)?,
            // §17.3.3.31 t, §17.3.3.7 delText (text of a deleted run)
            b"t" | b"delText" => {
                if s {
                    let t = read_text(r)?;
                    if !t.is_empty() {
                        run.content.push(RunContent::Text(t));
                    }
                }
            }
            b"tab" => run.content.push(RunContent::Tab), // §17.3.3.32
            // §17.3.3.1 br
            b"br" => run.content.push(match attr(e, b"type").as_deref() {
                Some("page") => RunContent::PageBreak,
                Some("column") => RunContent::ColumnBreak,
                _ => RunContent::LineBreak,
            }),
            b"cr" => run.content.push(RunContent::LineBreak), // §17.3.3.4
            b"noBreakHyphen" => run.content.push(RunContent::NoBreakHyphen),
            b"softHyphen" => run.content.push(RunContent::SoftHyphen),
            // §17.11.14 / §17.11.7
            b"footnoteReference" => run
                .content
                .push(RunContent::FootnoteRef(int(e, b"id").unwrap_or(0) as i32)),
            b"endnoteReference" => run
                .content
                .push(RunContent::EndnoteRef(int(e, b"id").unwrap_or(0) as i32)),
            b"footnoteRef" | b"endnoteRef" => run.content.push(RunContent::NoteMark),
            // §17.13.4.5 commentReference
            b"commentReference" => {
                if s {
                    r.skip_element()?;
                }
                run.content
                    .push(RunContent::CommentRef(int(e, b"id").unwrap_or(0) as i32));
            }
            // Rendered-page-break hints are caches; drop.
            b"lastRenderedPageBreak" => {}
            // §17.3.3.9 drawing
            b"drawing" if s => {
                let img = read_drawing(r, ctx, media)?;
                let raw = Raw::new(r.slice(before, r.pos()), 0);
                match img.or_else(|| drawing_shape(&raw.xml, ctx, media)) {
                    Some(img) => run
                        .content
                        .push(RunContent::Image(Box::new(with_src(img, raw)))),
                    None => run.content.push(RunContent::Opaque(raw)),
                }
            }
            // ECMA-376 Part 3, §10.2.1 AlternateContent: picture fallback, a shape
            // (DrawingML choice or VML fallback), else verbatim.
            b"AlternateContent" if s => {
                let raw = capture(r, true, before)?;
                match alternate_content_picture(&raw.xml, ctx, media)
                    .or_else(|| drawing_shape(&raw.xml, ctx, media))
                {
                    Some(img) => run
                        .content
                        .push(RunContent::Image(Box::new(with_src(img, raw)))),
                    None => run.content.push(RunContent::Opaque(raw)),
                }
            }
            // ECMA-376 Part 4, §19.1 VML picture container (legacy shapes / text boxes).
            b"pict" if s => {
                let raw = capture(r, true, before)?;
                match drawing_shape(&raw.xml, ctx, media) {
                    Some(img) => run
                        .content
                        .push(RunContent::Image(Box::new(with_src(img, raw)))),
                    None => run.content.push(RunContent::Opaque(raw)),
                }
            }
            // §17.16.18 fldChar
            _ => {
                let raw = capture(r, s, before)?;
                run.content.push(RunContent::Opaque(raw));
            }
        }
        Ok(true)
    })?;
    Ok(run)
}

fn read_text(r: &mut XmlReader<'_>) -> Result<String> {
    let end = r.depth() - 1;
    let mut s = String::new();
    loop {
        match r.read()? {
            Event::Text(t) => s.push_str(&unescape(t.as_ref())),
            Event::GeneralRef(g) => {
                s.push_str(&unescape(&format!("&{};", <_ as AsRef<str>>::as_ref(&g))));
            }
            Event::CData(t) => s.push_str(t.as_ref()),
            Event::End(_) if r.depth() == end => return Ok(s),
            Event::Eof => return Err(r.err("unexpected EOF in w:t").into()),
            _ => {}
        }
    }
}

/// Part name of the drawing fallback of a SmartArt whose data model is relationship `dm`.
fn diagram_drawing_part(ctx: &Ctx<'_>, dm: &str) -> Option<String> {
    let data = ctx.pkg.part(&ctx.target(dm)?)?;
    let mut r = XmlReader::new(&data.data, &data.name);
    loop {
        match r.read().ok()? {
            Event::Start(e) | Event::Empty(e) if name(&e) == b"dataModelExt" => {
                return attr(&e, b"relId").and_then(|id| ctx.target(&id));
            }
            Event::Eof => return None,
            _ => {}
        }
    }
}

/// `mc:AlternateContent` in a run (ECMA-376 Part 3, §10.2.1): when its `mc:Fallback`
/// holds a picture (e.g. the image fallback of a SmartArt or shape), show that.
fn alternate_content_picture(
    xml: &str,
    ctx: &Ctx<'_>,
    media: &mut BTreeMap<String, Arc<Vec<u8>>>,
) -> Option<Image> {
    let mut r = XmlReader::new(xml.as_bytes(), &ctx.part);
    let mut in_fallback = false;
    loop {
        match r.read().ok()? {
            Event::Start(e) if name(&e) == b"Fallback" => in_fallback = true,
            Event::Start(e) if in_fallback && name(&e) == b"drawing" => {
                return read_drawing(&mut r, ctx, media).ok().flatten();
            }
            Event::Eof => return None,
            _ => {}
        }
    }
}

/// Attach the source XML of a drawing object; shapes record their fingerprint so an
/// unchanged shape is written back verbatim.
fn with_src(mut img: Image, raw: Raw) -> Image {
    // A moved floating picture is written again (§20.4.2.3 anchor): the
    // fingerprint of the position as read.
    let fp = if img.shape.is_some() {
        crate::shape::shape_fingerprint(&img)
    } else {
        fingerprint(&img.anchor)
    };
    img.src = Some(Raw::new(raw.xml, fp));
    img
}

/// Theme part of the main document (`word/theme/theme1.xml`, ECMA-376 Part 1 §14.2.7);
/// its font scheme goes to [`Document::theme_fonts`].
fn read_theme(pkg: &Package, main: &str, doc: &mut Document) -> Option<Arc<Theme>> {
    let name = pkg.related_part(main, REL_THEME).ok()??;
    let part = pkg.part(&name)?;
    let t = bo_drawingml::theme::parse_theme(&part.data, &part.name).ok()?;
    let f = &t.fonts;
    doc.theme_fonts = ThemeFonts {
        major_latin: f.major.latin.typeface.clone(),
        major_east_asia: f.major.ea.typeface.clone(),
        major_cs: f.major.cs.typeface.clone(),
        minor_latin: f.minor.latin.typeface.clone(),
        minor_east_asia: f.minor.ea.typeface.clone(),
        minor_cs: f.minor.cs.typeface.clone(),
    };
    Some(Arc::new(t))
}

/// Text box content (§17.17.1 txbxContent) of the first text box in `xml`.
fn txbx_blocks(
    xml: &str,
    ctx: &Ctx<'_>,
    media: &mut BTreeMap<String, Arc<Vec<u8>>>,
) -> Option<Vec<Block>> {
    let mut r = XmlReader::new(xml.as_bytes(), &ctx.part);
    loop {
        match r.read().ok()? {
            Event::Start(e) if name(&e) == b"txbxContent" => {
                return read_blocks(&mut r, ctx, media, &mut None).ok();
            }
            Event::Empty(e) if name(&e) == b"txbxContent" => return Some(Vec::new()),
            Event::Eof => return None,
            _ => {}
        }
    }
}

/// A shape or text box (`wps:wsp`, else the VML fallback `w:pict`) in the XML of a
/// `w:drawing`, `mc:AlternateContent` or `w:pict` (see the `shape` module).
fn drawing_shape(
    xml: &str,
    ctx: &Ctx<'_>,
    media: &mut BTreeMap<String, Arc<Vec<u8>>>,
) -> Option<Image> {
    use crate::shape::{find_local, shape_from_wsp, vml_shape};
    let dom = bo_drawingml::dom::parse(xml.as_bytes(), &ctx.part).ok()?;
    // Groups and canvases (wpg:wgp, wpc:wpc) are not modelled: use the VML fallback.
    let grouped = find_local(&dom, "wgp").is_some() || find_local(&dom, "wpc").is_some();
    let wsp = (!grouped).then(|| find_local(&dom, "wsp")).flatten();
    let mut img = if let Some(wsp) = wsp {
        // Position and extent from the DrawingML container (§20.4.2.3 / §20.4.2.8).
        let mut r = XmlReader::new(xml.as_bytes(), &ctx.part);
        let mut img = None;
        loop {
            match r.read().ok()? {
                Event::Start(e) if name(&e) == b"drawing" => {
                    img = read_drawing_frame(&mut r, ctx, media).ok();
                    break;
                }
                Event::Eof => break,
                _ => {}
            }
        }
        let mut img = img?;
        img.shape = Some(Box::new(shape_from_wsp(wsp, ctx.theme.as_deref())));
        img
    } else {
        let pict = find_local(&dom, "pict")?;
        let (mut shape, w, h, anchor, rotation) = vml_shape(pict)?;
        // VML pictures (v:imagedata) stay verbatim unless they float, as picture
        // watermarks do (ECMA-376 Part 4, §19.1.2.11 imagedata).
        if find_local(pict, "imagedata").is_some() {
            let art = shape.art.as_mut()?;
            let media_name = ctx.target(art.picture.as_deref()?)?;
            anchor.as_ref()?;
            if !media.contains_key(&media_name) {
                let data = ctx.pkg.part(&media_name)?.data.clone();
                media.insert(media_name.clone(), Arc::new(data));
            }
            art.picture = Some(media_name);
        }
        Image {
            media: String::new(),
            width: w,
            height: h,
            anchor,
            crop: None,
            rotation,
            description: String::new(),
            shape: Some(Box::new(shape)),
            src: None,
        }
    };
    if let Some(s) = img.shape.as_mut() {
        s.text = txbx_blocks(xml, ctx, media);
    }
    Some(img)
}

/// `distT` `distB` `distL` `distR` of an anchor or wrap element (§20.4.2.3, §20.4.2.17):
/// the attributes present replace `base`.
fn wrap_dist(e: &BytesStart<'_>, base: [Emu; 4]) -> [Emu; 4] {
    let mut d = base;
    for (i, a) in [b"distT", b"distB", b"distL", b"distR"]
        .into_iter()
        .enumerate()
    {
        if let Some(v) = int(e, a) {
            d[i] = Emu(v.max(0));
        }
    }
    d
}

/// `w:drawing` → picture (ECMA-376 Part 1, §20.4 DrawingML WordprocessingML Drawing).
fn read_drawing(
    r: &mut XmlReader<'_>,
    ctx: &Ctx<'_>,
    media: &mut BTreeMap<String, Arc<Vec<u8>>>,
) -> Result<Option<Image>> {
    let img = read_drawing_frame(r, ctx, media)?;
    Ok(if img.media.is_empty() {
        None
    } else {
        Some(img)
    })
}

/// Placement (inline/anchor, extent, wrap) and picture data of a `w:drawing`; `media`
/// stays empty when it holds no picture (e.g. a shape).
fn read_drawing_frame(
    r: &mut XmlReader<'_>,
    ctx: &Ctx<'_>,
    media: &mut BTreeMap<String, Arc<Vec<u8>>>,
) -> Result<Image> {
    let end = r.depth() - 1;
    let mut img = Image {
        media: String::new(),
        width: Emu(0),
        height: Emu(0),
        anchor: None,
        crop: None,
        rotation: 0,
        description: String::new(),
        shape: None,
        src: None,
    };
    let mut anchor: Option<Anchor> = None;
    let mut in_polygon = false;
    let mut in_align = false;
    let mut axis = 0u8; // 1 = positionH, 2 = positionV
                        // SmartArt ([MS-ODRAWXML] dgm:relIds): data model part relationship id.
    let mut diagram_data: Option<String> = None;
    loop {
        let ev = r.read()?;
        let (e, is_start) = match &ev {
            Event::Start(e) => (e.clone(), true),
            Event::Empty(e) => (e.clone(), false),
            Event::End(e) => {
                let n = e.local_name().as_ref().as_bytes().to_vec();
                if n == b"positionH" || n == b"positionV" {
                    axis = 0;
                }
                if n == b"wrapPolygon" {
                    in_polygon = false;
                }
                if n == b"align" {
                    in_align = false;
                }
                if r.depth() == end {
                    break;
                }
                continue;
            }
            Event::Text(t) if axis != 0 => {
                let v = unescape(t.as_ref()).trim().to_owned();
                if let Some(a) = anchor.as_mut() {
                    if in_align {
                        // §20.4.2.1 / §20.4.2.2 align (ST_AlignH / ST_AlignV)
                        if axis == 1 {
                            a.h_align = Some(v);
                        } else {
                            a.v_align = Some(v);
                        }
                    } else if let Ok(v) = v.parse::<i64>() {
                        if axis == 1 {
                            a.x = Emu(v);
                        } else {
                            a.y = Emu(v);
                        }
                    }
                }
                continue;
            }
            Event::Eof => return Err(r.err("unexpected EOF in w:drawing").into()),
            _ => continue,
        };
        let _ = is_start;
        match name(&e).as_slice() {
            // §20.4.2.3 anchor
            b"anchor" => {
                anchor = Some(Anchor {
                    h_relative: "column".into(),
                    v_relative: "paragraph".into(),
                    behind: attr(&e, b"behindDoc").is_some_and(|v| v == "1" || v == "true"),
                    dist: wrap_dist(&e, [Emu(0); 4]),
                    ..Default::default()
                });
            }
            b"positionH" => {
                axis = 1;
                if let Some(a) = anchor.as_mut() {
                    a.h_relative = attr(&e, b"relativeFrom").unwrap_or_else(|| "column".into());
                }
            }
            b"positionV" => {
                axis = 2;
                if let Some(a) = anchor.as_mut() {
                    a.v_relative = attr(&e, b"relativeFrom").unwrap_or_else(|| "paragraph".into());
                }
            }
            b"wrapSquare" | b"wrapTight" | b"wrapThrough" | b"wrapTopAndBottom" | b"wrapNone" => {
                if let Some(a) = anchor.as_mut() {
                    a.wrap = match name(&e).as_slice() {
                        b"wrapSquare" => Wrap::Square,
                        b"wrapTight" | b"wrapThrough" => Wrap::Tight,
                        b"wrapTopAndBottom" => Wrap::TopAndBottom,
                        _ => Wrap::None,
                    };
                    // §20.4.3.7 ST_WrapText
                    a.wrap_text = match attr(&e, b"wrapText").as_deref() {
                        Some("left") => WrapText::Left,
                        Some("right") => WrapText::Right,
                        Some("largest") => WrapText::Largest,
                        _ => WrapText::BothSides,
                    };
                    a.dist = wrap_dist(&e, a.dist);
                }
            }
            // §20.4.2.16 wrapPolygon: its points (§20.4.2.14 start, §20.4.2.9 lineTo) and their bounding box.
            b"wrapPolygon" => in_polygon = is_start,
            b"start" | b"lineTo" if in_polygon => {
                if let (Some(a), Some(x), Some(y)) = (anchor.as_mut(), int(&e, b"x"), int(&e, b"y"))
                {
                    a.polygon_points.push((x, y));
                    a.polygon = Some(match a.polygon {
                        None => [x, y, x, y],
                        Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x), y1.max(y)],
                    });
                }
            }
            // §20.4.2.7 extent
            b"extent" => {
                img.width = Emu(int(&e, b"cx").unwrap_or(0));
                img.height = Emu(int(&e, b"cy").unwrap_or(0));
            }
            b"docPr" => img.description = attr(&e, b"descr").unwrap_or_default(),
            // §20.1.8.13 blip
            b"blip" => {
                if let Some(target) = attr(&e, b"embed").and_then(|id| ctx.target(&id)) {
                    if let Some(p) = ctx.pkg.part(&target) {
                        media
                            .entry(target.clone())
                            .or_insert_with(|| Arc::new(p.data.clone()));
                    }
                    img.media = target;
                }
            }
            // §20.1.8.55 srcRect
            b"srcRect" => {
                let g = |a: &[u8]| int(&e, a).unwrap_or(0) as i32;
                let c = [g(b"l"), g(b"t"), g(b"r"), g(b"b")];
                if c != [0; 4] {
                    img.crop = Some(c);
                }
            }
            // §20.1.7.6 xfrm
            b"xfrm" => img.rotation = int(&e, b"rot").unwrap_or(0) as i32,
            // ECMA-376 Part 1, §21.4.2.6 relIds (SmartArt parts).
            b"relIds" => diagram_data = attr(&e, b"dm"),
            b"align" => in_align = is_start,
            // Text box content is read separately; its pictures are not the drawing's.
            b"txbx" | b"txbxContent" if is_start => r.skip_element()?,
            _ => {}
        }
    }
    img.anchor = anchor;
    // SmartArt: show the pre-rendered drawing ([MS-ODRAWXML] §2.3.2 diagramDrawing,
    // referenced by `dsp:dataModelExt/@relId` of the data model part).
    if img.media.is_empty() {
        // Without a drawing part the data model itself is the "picture" (laid out when rendered);
        // either part is kept verbatim (an existing part is never rewritten on save).
        let part = diagram_data.as_deref().and_then(|dm| {
            diagram_drawing_part(ctx, dm)
                .filter(|d| ctx.pkg.part(d).is_some())
                .or_else(|| ctx.target(dm))
        });
        if let Some(part) = part {
            if let Some(p) = ctx.pkg.part(&part) {
                media
                    .entry(part.clone())
                    .or_insert_with(|| Arc::new(p.data.clone()));
                img.media = part;
            }
        }
    }
    Ok(img)
}

// ---------------------------------------------------------------- properties

type SectCtx<'c, 'p> = (
    &'c Ctx<'p>,
    &'c mut BTreeMap<String, Arc<Vec<u8>>>,
    &'c mut Option<Section>,
);

/// ECMA-376 Part 1, §17.3.1.26 pPr (Paragraph Properties).
fn read_ppr(r: &mut XmlReader<'_>, mut sect: Option<SectCtx<'_, '_>>) -> Result<ParaProps> {
    let mut p = ParaProps::default();
    children(r, |r, e, s, before| {
        match name(e).as_slice() {
            b"pStyle" => p.style = attr(e, b"val"), // §17.3.1.27
            b"keepNext" => p.keep_next = Some(on_off(e)),
            b"keepLines" => p.keep_lines = Some(on_off(e)),
            b"pageBreakBefore" => p.page_break_before = Some(on_off(e)), // §17.3.1.23
            b"widowControl" => p.widow_control = Some(on_off(e)),
            b"contextualSpacing" => p.contextual_spacing = Some(on_off(e)),
            b"bidi" => p.bidi = Some(on_off(e)), // §17.3.1.6
            b"outlineLvl" => p.outline_level = int(e, b"val").map(|v| v.clamp(0, 9) as u8),
            // §17.3.1.19 numPr
            b"numPr" if s => {
                let (mut id, mut lvl) = (None, 0u8);
                children(r, |_, e, s, _| {
                    match name(e).as_slice() {
                        b"numId" => id = int(e, b"val"),
                        b"ilvl" => lvl = int(e, b"val").unwrap_or(0).clamp(0, 8) as u8,
                        _ => {}
                    }
                    Ok(!s)
                })?;
                if let Some(id) = id {
                    p.num = Some(NumRef {
                        num_id: id as i32,
                        ilvl: lvl,
                    });
                }
            }
            // §17.3.1.13 jc
            b"jc" => p.align = align(attr(e, b"val").as_deref()).or(p.align),
            // §17.3.1.12 ind
            b"ind" => {
                p.indent_left = twips(e, b"left")
                    .or_else(|| twips(e, b"start"))
                    .or(p.indent_left);
                p.indent_right = twips(e, b"right")
                    .or_else(|| twips(e, b"end"))
                    .or(p.indent_right);
                if let Some(h) = twips(e, b"hanging") {
                    p.indent_first_line = Some(Twips(-h.0));
                } else if let Some(f) = twips(e, b"firstLine") {
                    p.indent_first_line = Some(f);
                }
            }
            // §17.3.1.33 spacing
            b"spacing" => {
                p.space_before = twips(e, b"before").or(p.space_before);
                p.space_after = twips(e, b"after").or(p.space_after);
                if let Some(line) = int(e, b"line") {
                    let line = line as i32;
                    p.line_spacing = Some(match attr(e, b"lineRule").as_deref() {
                        Some("exact") => LineSpacing::Exact(Twips(line.abs())),
                        Some("atLeast") => LineSpacing::AtLeast(Twips(line.abs())),
                        _ => LineSpacing::Auto(line.max(1) as u32),
                    });
                }
            }
            // §17.3.1.31 shd
            b"shd" => p.shading = color_attr(e, b"fill"),
            // §17.3.1.24 pBdr
            b"pBdr" if s => p.borders = Some(read_borders(r)?),
            // §17.3.1.38 tabs
            b"tabs" if s => {
                let mut v = Vec::new();
                children(r, |_, e, s, _| {
                    if name(e) == b"tab" {
                        if let Some(pos) = twips(e, b"pos") {
                            v.push(TabStop {
                                pos,
                                align: match attr(e, b"val").as_deref() {
                                    Some("center") => TabAlign::Center,
                                    Some("right" | "end") => TabAlign::Right,
                                    Some("decimal") => TabAlign::Decimal,
                                    Some("bar") => TabAlign::Bar,
                                    Some("clear") => TabAlign::Clear,
                                    _ => TabAlign::Left,
                                },
                                leader: match attr(e, b"leader").as_deref() {
                                    Some("dot" | "middleDot") => TabLeader::Dot,
                                    Some("hyphen") => TabLeader::Hyphen,
                                    Some("underscore" | "heavy") => TabLeader::Underscore,
                                    _ => TabLeader::None,
                                },
                            });
                        }
                    }
                    Ok(!s)
                })?;
                p.tabs = Some(v);
            }
            // §17.6.17 sectPr in the paragraph mark ends a section.
            b"sectPr" if s && sect.is_some() => {
                if let Some((ctx, media, out)) = sect.as_mut() {
                    let sec = read_sect_pr(r, ctx, media, before)?;
                    **out = Some(sec);
                }
            }
            // §17.13.5.29 pPrChange: paragraph properties before a tracked change.
            b"pPrChange" => {
                let info = rev_info(e);
                let mut old = ParaProps::default();
                if s {
                    children(r, |r, e, s, _| {
                        if s && name(e) == b"pPr" {
                            old = read_ppr(r, None)?;
                            return Ok(true);
                        }
                        Ok(!s)
                    })?;
                }
                p.change = Some(Box::new(ParaPropsChange { info, old }));
            }
            // §17.3.1.29 rPr of the paragraph mark: its tracked insertion/deletion
            // (§17.13.5.16 ins, §17.13.5.15 del, moveFrom/moveTo) is modelled, the
            // remaining children are kept verbatim.
            b"rPr" if s => {
                let tag_end = r.pos();
                let mut kept = String::new();
                let mut last = tag_end;
                children(r, |r, e, s, at| {
                    let n = name(e);
                    if matches!(n.as_slice(), b"ins" | b"del" | b"moveFrom" | b"moveTo") {
                        let mark = RevMark {
                            info: rev_info(e),
                            moved: matches!(n.as_slice(), b"moveFrom" | b"moveTo"),
                        };
                        if matches!(n.as_slice(), b"ins" | b"moveTo") {
                            p.mark_ins = Some(mark);
                        } else {
                            p.mark_del = Some(mark);
                        }
                        if s {
                            r.skip_element()?;
                        }
                    } else if n.as_slice() == b"lang" {
                        // §17.3.2.20 lang of the paragraph mark.
                        p.mark_lang = Some(read_lang(e));
                        if s {
                            r.skip_element()?;
                        }
                    } else if n.as_slice() == b"noProof" {
                        // §17.3.2.21 noProof of the paragraph mark.
                        p.mark_no_proof = Some(on_off(e));
                        if s {
                            r.skip_element()?;
                        }
                    } else {
                        kept.push_str(&capture(r, s, at)?.xml);
                    }
                    last = r.pos();
                    Ok(true)
                })?;
                let close = r.slice(last, r.pos());
                let close = close.trim_start();
                // An rPr that only held modelled children is regenerated from them.
                let modelled = p.mark_ins.is_some()
                    || p.mark_del.is_some()
                    || p.mark_lang.is_some()
                    || p.mark_no_proof.is_some();
                if !kept.is_empty() || !modelled {
                    let xml = format!("{}{kept}{close}", r.slice(before, tag_end));
                    p.unknown.push(Raw::new(xml, 0));
                }
            }
            _ => {
                let raw = capture(r, s, before)?;
                p.unknown.push(raw);
            }
        }
        Ok(true)
    })?;
    Ok(p)
}

fn read_border(e: &BytesStart<'_>) -> Border {
    Border {
        style: attr(e, b"val").unwrap_or_else(|| "single".into()),
        size: int(e, b"sz").unwrap_or(4).max(0) as u32,
        color: color_attr(e, b"color"),
        space: int(e, b"space").unwrap_or(0).max(0) as u32,
    }
}

/// Border containers (§17.3.1.24 pBdr, §17.4.39 tblBorders, §17.4.66 tcBorders).
fn read_borders(r: &mut XmlReader<'_>) -> Result<Borders> {
    let mut b = Borders::default();
    children(r, |_, e, s, _| {
        let v = Some(read_border(e));
        match name(e).as_slice() {
            b"top" => b.top = v,
            b"left" | b"start" => b.left = v,
            b"bottom" => b.bottom = v,
            b"right" | b"end" => b.right = v,
            b"insideH" | b"between" => b.inside_h = v,
            b"insideV" => b.inside_v = v,
            _ => {}
        }
        Ok(!s)
    })?;
    Ok(b)
}

/// ECMA-376 Part 1, §17.3.2.20 lang (`ST_Lang` tags for Latin, East Asian and complex script text).
fn read_lang(e: &BytesStart<'_>) -> Lang {
    Lang {
        val: attr(e, b"val"),
        east_asia: attr(e, b"eastAsia"),
        bidi: attr(e, b"bidi"),
    }
}

/// ECMA-376 Part 1, §17.3.2.28 rPr (Run Properties).
fn read_rpr(r: &mut XmlReader<'_>) -> Result<RunProps> {
    let mut p = RunProps::default();
    children(r, |r, e, s, before| {
        match name(e).as_slice() {
            b"rStyle" => p.style = attr(e, b"val"),
            // §17.3.2.26 rFonts
            b"rFonts" => {
                p.font = attr(e, b"ascii");
                p.font_h_ansi = attr(e, b"hAnsi");
                // §17.18.96 ST_Theme: theme fonts win over the explicit names.
                p.theme_ascii = attr(e, b"asciiTheme");
                p.theme_h_ansi = attr(e, b"hAnsiTheme");
                p.font_east_asia = attr(e, b"eastAsia");
                p.font_cs = attr(e, b"cs");
                p.theme_east_asia = attr(e, b"eastAsiaTheme");
                p.theme_cs = attr(e, b"cstheme");
                if p.font.is_none() && p.theme_ascii.is_none() {
                    // hAnsi alone: it also fills the ascii slot of the model.
                    p.font = p.font_h_ansi.take();
                    p.theme_ascii = p.theme_h_ansi.take();
                } else if (&p.font_h_ansi, &p.theme_h_ansi) == (&p.font, &p.theme_ascii) {
                    // Same as ascii: the model's "ascii stands for both".
                    p.font_h_ansi = None;
                    p.theme_h_ansi = None;
                }
                p.font_hint = attr(e, b"hint"); // §17.18.42 ST_Hint
                if s {
                    r.skip_element()?;
                }
                let _ = before;
            }
            // §17.3.2.38 sz
            b"sz" => {
                if let Some(v) = int(e, b"val") {
                    p.size = Some(HalfPoints(v.clamp(1, 3276) as u32));
                }
            }
            b"b" => p.bold = Some(on_off(e)),        // §17.3.2.1
            b"i" => p.italic = Some(on_off(e)),      // §17.3.2.16
            b"strike" => p.strike = Some(on_off(e)), // §17.3.2.37
            b"dstrike" => p.double_strike = Some(on_off(e)), // §17.3.2.9
            b"caps" => p.caps = Some(on_off(e)),     // §17.3.2.5
            b"smallCaps" => p.small_caps = Some(on_off(e)), // §17.3.2.33
            b"vanish" => p.hidden = Some(on_off(e)), // §17.3.2.41
            // §17.3.2.40 u
            b"u" => {
                // §17.18.99 ST_Underline (absent val = single)
                p.underline =
                    Some(attr(e, b"val").map_or(Underline::Single, |v| Underline::parse(&v)));
            }
            b"rtl" => p.rtl = Some(on_off(e)),    // §17.3.2.30
            b"cs" => p.complex = Some(on_off(e)), // §17.3.2.7
            b"lang" => {
                p.lang = Some(read_lang(e));
                if s {
                    r.skip_element()?;
                }
            }
            b"noProof" => p.no_proof = Some(on_off(e)), // §17.3.2.21
            // §17.3.2.6 color
            b"color" => {
                if let Some(c) = color_attr(e, b"val") {
                    p.color = Some(c);
                } else if attr(e, b"val").as_deref() == Some("auto") {
                    p.color = Some(Color::BLACK);
                }
            }
            b"highlight" => {
                p.highlight = attr(e, b"val").as_deref().and_then(Color::from_highlight);
            } // §17.3.2.15
            b"shd" => p.shading = color_attr(e, b"fill"), // §17.3.2.32
            // §17.3.2.42 vertAlign
            b"vertAlign" => {
                p.vert_align = match attr(e, b"val").as_deref() {
                    Some("superscript") => Some(VertAlign::Superscript),
                    Some("subscript") => Some(VertAlign::Subscript),
                    _ => Some(VertAlign::Baseline),
                };
            }
            b"spacing" => p.spacing = twips(e, b"val"), // §17.3.2.35
            b"kern" => p.kern = int(e, b"val").map(|v| HalfPoints(v.max(0) as u32)), // §17.3.2.19
            // §17.13.5.31 rPrChange: the run properties before a tracked formatting change.
            b"rPrChange" => {
                let info = rev_info(e);
                let mut old = RunProps::default();
                if s {
                    children(r, |r, e, s, _| {
                        if s && name(e) == b"rPr" {
                            old = read_rpr(r)?;
                            return Ok(true);
                        }
                        Ok(!s)
                    })?;
                }
                p.change = Some(Box::new(RunPropsChange { info, old }));
            }
            _ => {
                let raw = capture(r, s, before)?;
                p.unknown.push(raw);
            }
        }
        Ok(true)
    })?;
    Ok(p)
}

// ---------------------------------------------------------------- tables

fn table_width(e: &BytesStart<'_>) -> Option<TableWidth> {
    let w = int(e, b"w").unwrap_or(0);
    match attr(e, b"type").as_deref() {
        Some("pct") => {
            let raw = attr(e, b"w").unwrap_or_default();
            // §17.18.90: either fiftieths of a percent or "NN%".
            if let Some(p) = raw.strip_suffix('%') {
                p.parse::<f64>()
                    .ok()
                    .map(|v| TableWidth::Pct((v * 50.0) as u32))
            } else {
                Some(TableWidth::Pct(w.max(0) as u32))
            }
        }
        Some("auto" | "nil") => Some(TableWidth::Auto),
        _ => Some(TableWidth::Dxa(Twips(w as i32))),
    }
}

/// §17.4.60 tblPr
fn read_tblpr(r: &mut XmlReader<'_>) -> Result<TableProps> {
    let mut t = TableProps::default();
    children(r, |r, e, s, before| {
        match name(e).as_slice() {
            b"tblStyle" => t.style = attr(e, b"val"),
            b"tblW" => t.width = table_width(e),
            b"jc" => t.align = align(attr(e, b"val").as_deref()),
            b"tblInd" => t.indent = twips(e, b"w"),
            b"tblBorders" if s => t.borders = Some(read_borders(r)?),
            b"tblLayout" => t.fixed_layout = attr(e, b"type").as_deref() == Some("fixed"),
            // ECMA-376 Part 1, §17.4.58 tblpPr (floating table position).
            b"tblpPr" => {
                let tw = |a: &[u8]| twips(e, a).unwrap_or(Twips(0));
                t.float = Some(TableFloat {
                    left_from_text: tw(b"leftFromText"),
                    right_from_text: tw(b"rightFromText"),
                    top_from_text: tw(b"topFromText"),
                    bottom_from_text: tw(b"bottomFromText"),
                    horz_anchor: attr(e, b"horzAnchor"),
                    vert_anchor: attr(e, b"vertAnchor"),
                    x: tw(b"tblpX"),
                    x_align: attr(e, b"tblpXSpec"),
                    y: tw(b"tblpY"),
                    y_align: attr(e, b"tblpYSpec"),
                });
                return Ok(!s);
            }
            // §17.13.5.34 tblPrChange
            b"tblPrChange" if s => {
                let info = rev_info(e);
                let mut old = TableProps::default();
                children(r, |r, e, s, _| {
                    if s && name(e) == b"tblPr" {
                        old = read_tblpr(r)?;
                        return Ok(true);
                    }
                    Ok(!s)
                })?;
                t.change = Some(Box::new(TablePropsChange { info, old }));
            }
            b"tblCellMar" if s => {
                let (mut l, mut rr) = (Twips(108), Twips(108));
                children(r, |_, e, s, _| {
                    match name(e).as_slice() {
                        b"left" | b"start" => l = twips(e, b"w").unwrap_or(l),
                        b"right" | b"end" => rr = twips(e, b"w").unwrap_or(rr),
                        _ => {}
                    }
                    Ok(!s)
                })?;
                t.cell_margin = Some((l, rr));
            }
            _ => t.unknown.push(capture(r, s, before)?),
        }
        Ok(true)
    })?;
    Ok(t)
}

/// §17.4.38 tbl
fn read_table(
    r: &mut XmlReader<'_>,
    ctx: &Ctx<'_>,
    media: &mut BTreeMap<String, Arc<Vec<u8>>>,
) -> Result<Table> {
    let mut t = Table::default();
    children(r, |r, e, s, _| {
        match name(e).as_slice() {
            b"tblPr" if s => t.props = read_tblpr(r)?,
            b"tblGrid" if s => {
                children(r, |r, e, s, _| {
                    match name(e).as_slice() {
                        b"gridCol" => t.grid.push(twips(e, b"w").unwrap_or(Twips(0))),
                        // §17.13.5.33 tblGridChange: the previous tblGrid.
                        b"tblGridChange" if s => {
                            let info = rev_info(e);
                            let mut old = Vec::new();
                            children(r, |r, e, s, _| {
                                if s && name(e) == b"tblGrid" {
                                    children(r, |_, e, s, _| {
                                        if name(e) == b"gridCol" {
                                            old.push(twips(e, b"w").unwrap_or(Twips(0)));
                                        }
                                        Ok(!s)
                                    })?;
                                    return Ok(true);
                                }
                                Ok(!s)
                            })?;
                            t.grid_change = Some(Box::new(GridChange { info, old }));
                            return Ok(true);
                        }
                        _ => {}
                    }
                    Ok(!s)
                })?;
            }
            // §17.4.79 tr
            b"tr" if s => {
                let mut row = Row::default();
                children(r, |r, e, s, before| {
                    match name(e).as_slice() {
                        b"trPr" if s => {
                            children(r, |r, e, s, before| {
                                match name(e).as_slice() {
                                    b"tblHeader" => row.props.header = on_off(e),
                                    // §17.13.5.19 ins / §17.13.5.12 del (tracked rows)
                                    b"ins" => {
                                        row.props.ins = Some(RevMark {
                                            info: rev_info(e),
                                            moved: false,
                                        });
                                        return Ok(!s);
                                    }
                                    b"del" => {
                                        row.props.del = Some(RevMark {
                                            info: rev_info(e),
                                            moved: false,
                                        });
                                        return Ok(!s);
                                    }
                                    b"cantSplit" => row.props.cant_split = on_off(e),
                                    b"trHeight" => row.props.height = tr_height(e),
                                    // §17.13.5.37 trPrChange (previous CT_TrPrBase)
                                    b"trPrChange" if s => {
                                        let mut ch = RowPropsChange {
                                            info: rev_info(e),
                                            ..RowPropsChange::default()
                                        };
                                        children(r, |r, e, s, _| {
                                            if s && name(e) == b"trPr" {
                                                children(r, |_, e, s, _| {
                                                    match name(e).as_slice() {
                                                        b"tblHeader" => ch.header = on_off(e),
                                                        b"cantSplit" => ch.cant_split = on_off(e),
                                                        b"trHeight" => ch.height = tr_height(e),
                                                        _ => {}
                                                    }
                                                    Ok(!s)
                                                })?;
                                                return Ok(true);
                                            }
                                            Ok(!s)
                                        })?;
                                        row.props.change = Some(Box::new(ch));
                                    }
                                    _ => row.props.unknown.push(capture(r, s, before)?),
                                }
                                Ok(true)
                            })?;
                        }
                        // §17.4.66 tc
                        b"tc" if s => {
                            let mut cell = Cell::default();
                            let end = r.depth() - 1;
                            loop {
                                let before = r.pos();
                                match r.read()? {
                                    Event::Start(e) if name(&e) == b"tcPr" => {
                                        cell.props = read_tcpr(r)?;
                                    }
                                    Event::Start(e) => {
                                        let mut sect = None;
                                        cell.blocks.extend(read_block(
                                            r, &e, ctx, media, before, &mut sect,
                                        )?);
                                    }
                                    Event::Empty(e) => {
                                        let raw = Raw::new(r.slice(before, r.pos()), 0);
                                        if name(&e) == b"p" {
                                            let mut p = Paragraph::default();
                                            p.src =
                                                Some(Raw::new(raw.xml, p.content_fingerprint()));
                                            cell.blocks.push(Block::Paragraph(p));
                                        } else if name(&e) != b"tcPr" {
                                            cell.blocks.push(Block::Opaque(raw));
                                        }
                                    }
                                    Event::End(_) if r.depth() == end => break,
                                    Event::Eof => {
                                        return Err(r.err("unexpected EOF in w:tc").into())
                                    }
                                    _ => {}
                                }
                            }
                            row.cells.push(cell);
                        }
                        _ => {
                            let raw = capture(r, s, before)?;
                            row.props.unknown.push(raw);
                        }
                    }
                    Ok(true)
                })?;
                fold_h_merge(&mut row);
                t.rows.push(row);
            }
            _ => return Ok(!s),
        }
        Ok(true)
    })?;
    if t.grid.is_empty() {
        let cols = t
            .rows
            .iter()
            .map(|r| {
                r.cells
                    .iter()
                    .map(|c| c.props.grid_span.max(1) as usize)
                    .sum::<usize>()
            })
            .max()
            .unwrap_or(1);
        t.grid = vec![Twips(9000 / cols.max(1) as i32); cols];
    }
    Ok(t)
}

/// `w:hMerge` of a cell (§17.4.22, legacy horizontal merge): `Some(true)` = restart,
/// `Some(false)` = continue (the default value).
fn h_merge_of(c: &Cell) -> Option<bool> {
    c.props.unknown.iter().find_map(|r| {
        let x = r.xml.trim_start();
        let rest = x.strip_prefix('<')?;
        let local = rest.split([' ', '/', '>']).next()?;
        (local.rsplit(':').next() == Some("hMerge")).then(|| x.contains("\"restart\""))
    })
}

/// Fold legacy horizontal merges (§17.4.22 hMerge) into `gridSpan` (§17.4.17): a
/// continuing cell is absorbed by the cell before it, so layout and editing only see
/// the modern form. Unchanged tables are still written from their source XML.
fn fold_h_merge(row: &mut Row) {
    if !row.cells.iter().any(|c| h_merge_of(c).is_some()) {
        return;
    }
    let mut out: Vec<Cell> = Vec::with_capacity(row.cells.len());
    for mut c in std::mem::take(&mut row.cells) {
        let hm = h_merge_of(&c);
        c.props.unknown.retain(|r| !r.xml.contains("hMerge"));
        match (hm, out.last_mut()) {
            (Some(false), Some(prev)) => {
                prev.props.grid_span += c.props.grid_span.max(1);
                if let (Some(TableWidth::Dxa(a)), Some(TableWidth::Dxa(b))) =
                    (prev.props.width, c.props.width)
                {
                    prev.props.width = Some(TableWidth::Dxa(Twips(a.0 + b.0)));
                }
                // Text left in a continuing cell joins the merged cell.
                let has_text = c
                    .blocks
                    .iter()
                    .any(|b| b.as_paragraph().is_none_or(|p| !p.plain_text().is_empty()));
                if has_text {
                    prev.blocks.extend(c.blocks);
                }
            }
            _ => out.push(c),
        }
    }
    row.cells = out;
}

/// §17.4.81 trHeight (`hRule` §17.18.37; `auto` is read as at least).
fn tr_height(e: &BytesStart<'_>) -> Option<(Twips, HeightRule)> {
    twips(e, b"val").map(|h| {
        (
            h,
            if attr(e, b"hRule").as_deref() == Some("exact") {
                HeightRule::Exact
            } else {
                HeightRule::AtLeast
            },
        )
    })
}

/// §17.4.70 tcPr
fn read_tcpr(r: &mut XmlReader<'_>) -> Result<CellProps> {
    let mut c = CellProps::default();
    children(r, |r, e, s, before| {
        match name(e).as_slice() {
            b"tcW" => c.width = table_width(e),
            b"gridSpan" => c.grid_span = int(e, b"val").unwrap_or(1).clamp(1, 64) as u32,
            b"vMerge" => {
                c.v_merge = Some(if attr(e, b"val").as_deref() == Some("restart") {
                    VMerge::Restart
                } else {
                    VMerge::Continue
                });
            }
            b"shd" => c.shading = color_attr(e, b"fill"),
            b"tcBorders" if s => c.borders = Some(read_borders(r)?),
            // §17.13.5.4 cellIns / §17.13.5.3 cellDel
            n @ (b"cellIns" | b"cellDel") => {
                let m = RevMark {
                    info: rev_info(e),
                    moved: false,
                };
                if n == b"cellIns" {
                    c.ins = Some(m);
                } else {
                    c.del = Some(m);
                }
                return Ok(!s);
            }
            // §17.13.5.6 cellMerge (§17.18.1 ST_AnnotationVMerge: rest / cont)
            b"cellMerge" => {
                let vm = |a: &[u8]| match attr(e, a).as_deref() {
                    Some("rest") => Some(VMerge::Restart),
                    Some("cont") => Some(VMerge::Continue),
                    _ => None,
                };
                c.merge = Some(Box::new(CellMerge {
                    info: rev_info(e),
                    v_merge: vm(b"vMerge"),
                    v_merge_orig: vm(b"vMergeOrig"),
                }));
                return Ok(!s);
            }
            // §17.13.5.36 tcPrChange
            b"tcPrChange" if s => {
                let info = rev_info(e);
                let mut old = CellProps::default();
                children(r, |r, e, s, _| {
                    if s && name(e) == b"tcPr" {
                        old = read_tcpr(r)?;
                        return Ok(true);
                    }
                    Ok(!s)
                })?;
                c.change = Some(Box::new(CellPropsChange { info, old }));
            }
            b"vAlign" => {
                c.v_align = match attr(e, b"val").as_deref() {
                    Some("center") => Some(CellVAlign::Center),
                    Some("bottom") => Some(CellVAlign::Bottom),
                    _ => Some(CellVAlign::Top),
                };
            }
            // ECMA-376 Part 1, §17.4.72 textDirection (§17.18.93 ST_TextDirection).
            b"textDirection"
                if attr(e, b"val")
                    .as_deref()
                    .and_then(TextDirection::parse)
                    .is_some() =>
            {
                c.text_direction = attr(e, b"val").as_deref().and_then(TextDirection::parse);
                return Ok(!s);
            }
            _ => c.unknown.push(capture(r, s, before)?),
        }
        Ok(true)
    })?;
    Ok(c)
}

// ---------------------------------------------------------------- sections

/// ECMA-376 Part 1, §17.6.17 sectPr.
fn read_sect_pr(
    r: &mut XmlReader<'_>,
    ctx: &Ctx<'_>,
    media: &mut BTreeMap<String, Arc<Vec<u8>>>,
    before: usize,
) -> Result<Section> {
    let mut s = Section::default();
    let mut hf: Vec<(bool, String, String)> = Vec::new(); // (is_header, type, rId)
    children(r, |rr, e, st, _| {
        match name(e).as_slice() {
            // §17.10.5 headerReference / §17.10.3 footerReference
            n @ (b"headerReference" | b"footerReference") => {
                if let Some(id) = attr(e, b"id") {
                    hf.push((
                        n == b"headerReference",
                        attr(e, b"type").unwrap_or_else(|| "default".into()),
                        id,
                    ));
                }
            }
            // §17.6.13 pgSz
            b"pgSz" => {
                s.page.width = twips(e, b"w").filter(|t| t.0 > 0).unwrap_or(s.page.width);
                s.page.height = twips(e, b"h").filter(|t| t.0 > 0).unwrap_or(s.page.height);
            }
            // §17.6.11 pgMar
            b"pgMar" => {
                let p = &mut s.page;
                p.margin_top = twips(e, b"top").map_or(p.margin_top, |t| Twips(t.0.abs()));
                p.margin_bottom = twips(e, b"bottom").map_or(p.margin_bottom, |t| Twips(t.0.abs()));
                p.margin_left = twips(e, b"left").unwrap_or(p.margin_left);
                p.margin_right = twips(e, b"right").unwrap_or(p.margin_right);
                p.header = twips(e, b"header").unwrap_or(p.header);
                p.footer = twips(e, b"footer").unwrap_or(p.footer);
            }
            // §17.6.4 cols
            b"cols" => {
                s.columns = Columns {
                    count: int(e, b"num").unwrap_or(1).clamp(1, 45) as u16,
                    space: twips(e, b"space").unwrap_or(Twips(720)),
                };
            }
            b"titlePg" => s.title_page = on_off(e),
            // §17.6.6 formProt
            b"formProt" => s.form_unprotected = !on_off(e),
            // §17.6.22 type
            b"type" => {
                s.break_type = match attr(e, b"val").as_deref() {
                    Some("continuous") => SectionBreak::Continuous,
                    Some("evenPage") => SectionBreak::EvenPage,
                    Some("oddPage") => SectionBreak::OddPage,
                    Some("nextColumn") => SectionBreak::NextColumn,
                    _ => SectionBreak::NextPage,
                };
            }
            // §17.6.12 pgNumType
            b"pgNumType" => s.page_number_start = int(e, b"start").map(|v| v.max(0) as u32),
            // §17.6.10 pgBorders
            b"pgBorders" => {
                let mut pb = PageBorders {
                    // §17.18.64 ST_PageBorderOffset (default text)
                    offset_from_page: attr(e, b"offsetFrom").as_deref() == Some("page"),
                    // §17.18.63 ST_PageBorderDisplay (default allPages)
                    display: PageBorderDisplay::parse(attr(e, b"display").as_deref().unwrap_or("")),
                    // §17.18.65 ST_PageBorderZOrder (default front)
                    behind: attr(e, b"zOrder").as_deref() == Some("back"),
                    ..PageBorders::default()
                };
                if st {
                    children(rr, |_, e, st, _| {
                        let flag = |a: &[u8]| attr(e, a).is_some_and(|v| on_off_value(&v));
                        // §17.6.21 top, §17.6.7 left, §17.6.2 bottom, §17.6.15 right
                        let edge = Some(PageBorder {
                            line: read_border(e),
                            shadow: flag(b"shadow"),
                            frame: flag(b"frame"),
                        });
                        match name(e).as_slice() {
                            b"top" => pb.top = edge,
                            b"left" | b"start" => pb.left = edge,
                            b"bottom" => pb.bottom = edge,
                            b"right" | b"end" => pb.right = edge,
                            _ => {}
                        }
                        Ok(!st)
                    })?;
                }
                s.page_borders = Some(pb);
                return Ok(true);
            }
            // §17.13.5.32 sectPrChange
            b"sectPrChange" if st => {
                let info = rev_info(e);
                let mut old = None;
                children(rr, |rr, e, st, before| {
                    if st && name(e) == b"sectPr" {
                        old = Some(read_sect_pr(rr, ctx, media, before)?);
                        return Ok(true);
                    }
                    Ok(!st)
                })?;
                if let Some(o) = old {
                    s.change = Some(Box::new(o.change_snapshot(info)));
                }
                return Ok(true);
            }
            // §17.11.11 footnotePr / §17.11.4 endnotePr
            n @ (b"footnotePr" | b"endnotePr") if st => {
                let np = read_note_pr(rr)?;
                if n == b"footnotePr" {
                    s.footnote_pr = np;
                } else {
                    s.endnote_pr = np;
                }
                return Ok(true);
            }
            _ => {}
        }
        Ok(!st)
    })?;
    for (is_header, kind, id) in hf {
        let Some(target) = ctx.target(&id) else {
            continue;
        };
        let Some(part) = ctx.pkg.part(&target) else {
            continue;
        };
        let mut hctx = Ctx::new(ctx.pkg, &target)?;
        hctx.theme.clone_from(&ctx.theme);
        let blocks = read_header_part(&part.data, &hctx, media)?;
        s.hf_parts.0.push((is_header, kind.clone(), target.clone()));
        let set = if is_header {
            &mut s.headers
        } else {
            &mut s.footers
        };
        match kind.as_str() {
            "first" => set.first = Some(blocks),
            "even" => set.even = Some(blocks),
            _ => set.default = Some(blocks),
        }
    }
    s.src = Some(Raw::new(r.slice(before, r.pos()), s.props_fingerprint()));
    Ok(s)
}

/// Children of §17.11.11 footnotePr / §17.11.4 endnotePr in a section.
fn read_note_pr(r: &mut XmlReader<'_>) -> Result<NoteProps> {
    let mut np = NoteProps::default();
    children(r, |_, e, st, _| {
        let val = attr(e, b"val");
        match name(e).as_slice() {
            // §17.11.21 / §17.11.22 pos
            b"pos" => np.pos = val.as_deref().and_then(NotePos::parse),
            // §17.11.17 numFmt
            b"numFmt" => np.fmt = val.as_deref().map(NumFormat::parse),
            // §17.11.20 numStart
            b"numStart" => np.start = int(e, b"val").map(|v| v.max(0) as u32),
            // §17.11.19 numRestart
            b"numRestart" => np.restart = val.as_deref().and_then(NoteRestart::parse),
            _ => {}
        }
        Ok(!st)
    })?;
    Ok(np)
}

/// §17.10.4 hdr / §17.10.2 ftr
fn read_header_part(
    data: &[u8],
    ctx: &Ctx<'_>,
    media: &mut BTreeMap<String, Arc<Vec<u8>>>,
) -> Result<Vec<Block>> {
    let mut r = XmlReader::new(data, &ctx.part);
    loop {
        match r.read()? {
            Event::Start(e) if matches!(name(&e).as_slice(), b"hdr" | b"ftr") => {
                return read_blocks(&mut r, ctx, media, &mut None);
            }
            Event::Eof => return Ok(Vec::new()),
            _ => {}
        }
    }
}

/// Exposed for the writer: all header/footer sets of a document as (is_header, kind, blocks).
pub(crate) fn header_footer_sets(s: &Section) -> Vec<(bool, &'static str, &Vec<Block>)> {
    fn push<'s>(v: &mut Vec<(bool, &'static str, &'s Vec<Block>)>, h: bool, set: &'s HeaderFooter) {
        if let Some(b) = &set.default {
            v.push((h, "default", b));
        }
        if let Some(b) = &set.first {
            v.push((h, "first", b));
        }
        if let Some(b) = &set.even {
            v.push((h, "even", b));
        }
    }
    let mut v = Vec::new();
    push(&mut v, true, &s.headers);
    push(&mut v, false, &s.footers);
    v
}
