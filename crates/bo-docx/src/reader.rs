//! `.docx` → `bo_model::Document`.

use crate::{DocxError, Result};
use bo_model::{
    Alignment, Block, Color, Document, HalfPoints, LineSpacing, PageSetup, ParaProps, Paragraph,
    Run, RunContent, RunProps, Section, Twips, Underline,
};
use bo_opc::xml::{attr, unescape, XmlReader};
use bo_opc::{Limits, Package, REL_OFFICE_DOCUMENT, REL_STYLES};
use quick_xml::events::{BytesStart, Event};

/// Read a `.docx` file into the document model.
pub fn read_docx(bytes: &[u8]) -> Result<Document> {
    let pkg = Package::read(bytes, &Limits::default())?;
    let main = pkg
        .related_part("/", REL_OFFICE_DOCUMENT)?
        .ok_or(DocxError::NoMainDocument)?;
    let main_part = pkg.part(&main).ok_or(DocxError::NoMainDocument)?;

    let mut doc = Document::default();
    if let Some(styles) = pkg.related_part(&main, REL_STYLES)? {
        if let Some(p) = pkg.part(&styles) {
            read_doc_defaults(&p.data, &styles, &mut doc)?;
        }
    }
    read_body(&main_part.data, &main, &mut doc)?;
    Ok(doc)
}

/// ECMA-376 Part 1, §17.7.5.1 docDefaults (rPrDefault §17.7.5.5, pPrDefault §17.7.5.3).
fn read_doc_defaults(data: &[u8], part: &str, doc: &mut Document) -> Result<()> {
    let mut r = XmlReader::new(data, part);
    loop {
        match r.next()? {
            Event::Start(e) => match e.local_name().as_ref().as_bytes() {
                b"rPrDefault" => {
                    if let Some(p) = read_wrapped_rpr(&mut r)? {
                        doc.default_run = p;
                    }
                }
                b"pPrDefault" => {
                    if let Some(p) = read_wrapped_ppr(&mut r)? {
                        doc.default_para = p;
                    }
                }
                // Phase 1 only reads defaults; skip the style definitions.
                b"style" | b"latentStyles" => r.skip_element()?,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(())
}

fn read_wrapped_rpr(r: &mut XmlReader<'_>) -> Result<Option<RunProps>> {
    let end = r.depth() - 1;
    let mut out = None;
    loop {
        match r.next()? {
            Event::Start(e) if e.local_name().as_ref().as_bytes() == b"rPr" => out = Some(read_rpr(r)?),
            Event::End(_) if r.depth() == end => return Ok(out),
            Event::Eof => return Err(r.err("unexpected EOF").into()),
            _ => {}
        }
    }
}

fn read_wrapped_ppr(r: &mut XmlReader<'_>) -> Result<Option<ParaProps>> {
    let end = r.depth() - 1;
    let mut out = None;
    loop {
        match r.next()? {
            Event::Start(e) if e.local_name().as_ref().as_bytes() == b"pPr" => out = Some(read_ppr(r, None)?),
            Event::End(_) if r.depth() == end => return Ok(out),
            Event::Eof => return Err(r.err("unexpected EOF").into()),
            _ => {}
        }
    }
}

/// ECMA-376 Part 1, §17.2.2 body.
fn read_body(data: &[u8], part: &str, doc: &mut Document) -> Result<()> {
    let mut r = XmlReader::new(data, part);
    let mut blocks: Vec<Block> = Vec::new();
    loop {
        match r.next()? {
            Event::Start(e) => match e.local_name().as_ref().as_bytes() {
                b"p" => {
                    let mut sect = None;
                    let p = read_paragraph(&mut r, &mut sect)?;
                    blocks.push(Block::Paragraph(p));
                    // §17.6.17: a sectPr inside the last paragraph's pPr ends a section.
                    if let Some(page) = sect {
                        doc.sections.push(Section {
                            page,
                            blocks: std::mem::take(&mut blocks),
                        });
                    }
                }
                // §17.6.17 sectPr — final section properties of the body.
                b"sectPr" => {
                    let page = read_sect_pr(&mut r)?;
                    doc.sections.push(Section {
                        page,
                        blocks: std::mem::take(&mut blocks),
                    });
                }
                // Tables, SDTs etc. are Phase 2: skip for now.
                b"tbl" | b"sdt" | b"customXml" => r.skip_element()?,
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
    }
    if !blocks.is_empty() || doc.sections.is_empty() {
        doc.sections.push(Section {
            page: PageSetup::default(),
            blocks,
        });
    }
    Ok(())
}

/// ECMA-376 Part 1, §17.3.1.22 p.
fn read_paragraph(r: &mut XmlReader<'_>, sect: &mut Option<PageSetup>) -> Result<Paragraph> {
    let end = r.depth() - 1;
    let mut p = Paragraph::default();
    loop {
        match r.next()? {
            Event::Start(e) => match e.local_name().as_ref().as_bytes() {
                b"pPr" => p.props = read_ppr(r, Some(sect))?,
                b"r" => p.runs.push(read_run(r)?),
                // §17.16.22 hyperlink, §17.3.2.30 smartTag, ins (§17.13.5.18): descend into children.
                b"hyperlink" | b"smartTag" | b"ins" | b"fldSimple" => {}
                // §17.13.5.14 del: deleted content is not displayed.
                b"del" | b"moveFrom" => r.skip_element()?,
                _ => {}
            },
            Event::End(_) if r.depth() == end => return Ok(p),
            Event::Eof => return Err(r.err("unexpected EOF in w:p").into()),
            _ => {}
        }
    }
}

fn on_off(e: &BytesStart<'_>) -> bool {
    // §17.17.4 ST_OnOff: absent val means true.
    !matches!(attr(e, b"val").as_deref(), Some("0" | "false" | "off"))
}

fn twips(e: &BytesStart<'_>, name: &[u8]) -> Option<Twips> {
    attr(e, name).and_then(|v| v.parse::<f64>().ok()).map(|v| Twips(v as i32))
}

/// ECMA-376 Part 1, §17.3.1.26 pPr (Paragraph Properties).
fn read_ppr(r: &mut XmlReader<'_>, mut sect: Option<&mut Option<PageSetup>>) -> Result<ParaProps> {
    let end = r.depth() - 1;
    let mut p = ParaProps::default();
    loop {
        let ev = r.next()?;
        let (e, is_start) = match &ev {
            Event::Start(e) => (e.clone(), true),
            Event::Empty(e) => (e.clone(), false),
            Event::End(_) if r.depth() == end => return Ok(p),
            Event::Eof => return Err(r.err("unexpected EOF in w:pPr").into()),
            _ => continue,
        };
        match e.local_name().as_ref().as_bytes() {
            // §17.3.1.13 jc
            b"jc" => {
                p.align = match attr(&e, b"val").as_deref() {
                    Some("center") => Some(Alignment::Center),
                    Some("right" | "end") => Some(Alignment::Right),
                    Some("both" | "distribute") => Some(Alignment::Justify),
                    Some("left" | "start") => Some(Alignment::Left),
                    _ => p.align,
                };
            }
            // §17.3.1.12 ind
            b"ind" => {
                p.indent_left = twips(&e, b"left").or_else(|| twips(&e, b"start")).or(p.indent_left);
                p.indent_right = twips(&e, b"right").or_else(|| twips(&e, b"end")).or(p.indent_right);
                if let Some(h) = twips(&e, b"hanging") {
                    p.indent_first_line = Some(Twips(-h.0));
                } else if let Some(f) = twips(&e, b"firstLine") {
                    p.indent_first_line = Some(f);
                }
            }
            // §17.3.1.33 spacing
            b"spacing" => {
                p.space_before = twips(&e, b"before").or(p.space_before);
                p.space_after = twips(&e, b"after").or(p.space_after);
                if let Some(line) = attr(&e, b"line").and_then(|v| v.parse::<i32>().ok()) {
                    p.line_spacing = Some(match attr(&e, b"lineRule").as_deref() {
                        Some("exact") => LineSpacing::Exact(Twips(line.abs())),
                        Some("atLeast") => LineSpacing::AtLeast(Twips(line.abs())),
                        _ => LineSpacing::Auto(line.max(1) as u32),
                    });
                }
            }
            // §17.3.1.23 pageBreakBefore
            b"pageBreakBefore" => p.page_break_before = Some(on_off(&e)),
            b"sectPr" if is_start => {
                let page = read_sect_pr(r)?;
                if let Some(s) = sect.as_deref_mut() {
                    *s = Some(page);
                }
            }
            // §17.3.1.29 rPr (paragraph mark run properties) — not needed for layout yet.
            b"rPr" | b"numPr" | b"tabs" | b"pBdr" | b"pPrChange" if is_start => r.skip_element()?,
            _ => {}
        }
    }
}

/// ECMA-376 Part 1, §17.3.2.28 rPr (Run Properties).
fn read_rpr(r: &mut XmlReader<'_>) -> Result<RunProps> {
    let end = r.depth() - 1;
    let mut p = RunProps::default();
    loop {
        let ev = r.next()?;
        let (e, is_start) = match &ev {
            Event::Start(e) => (e.clone(), true),
            Event::Empty(e) => (e.clone(), false),
            Event::End(_) if r.depth() == end => return Ok(p),
            Event::Eof => return Err(r.err("unexpected EOF in w:rPr").into()),
            _ => continue,
        };
        match e.local_name().as_ref().as_bytes() {
            // §17.3.2.26 rFonts — Phase 1 uses ascii, then hAnsi.
            b"rFonts" => {
                if let Some(f) = attr(&e, b"ascii").or_else(|| attr(&e, b"hAnsi")) {
                    p.font = Some(f);
                }
            }
            // §17.3.2.38 sz
            b"sz" => {
                if let Some(v) = attr(&e, b"val").and_then(|v| v.parse::<f32>().ok()) {
                    p.size = Some(HalfPoints(v.round().clamp(1.0, 3276.0) as u32));
                }
            }
            b"b" => p.bold = Some(on_off(&e)),   // §17.3.2.1
            b"i" => p.italic = Some(on_off(&e)), // §17.3.2.16
            b"strike" => p.strike = Some(on_off(&e)), // §17.3.2.37
            // §17.3.2.40 u
            b"u" => {
                p.underline = Some(match attr(&e, b"val").as_deref() {
                    Some("none") => Underline::None,
                    Some("double") => Underline::Double,
                    _ => Underline::Single,
                });
            }
            // §17.3.2.6 color
            b"color" => {
                if let Some(c) = attr(&e, b"val").as_deref().and_then(Color::from_hex) {
                    p.color = Some(c);
                }
            }
            b"rPrChange" if is_start => r.skip_element()?,
            _ => {}
        }
    }
}

/// ECMA-376 Part 1, §17.3.2.25 r.
fn read_run(r: &mut XmlReader<'_>) -> Result<Run> {
    let end = r.depth() - 1;
    let mut run = Run::default();
    loop {
        match r.next()? {
            Event::Start(e) => match e.local_name().as_ref().as_bytes() {
                b"rPr" => run.props = read_rpr(r)?,
                // §17.3.3.31 t
                b"t" => {
                    let text = read_text(r)?;
                    if !text.is_empty() {
                        run.content.push(RunContent::Text(text));
                    }
                }
                // Drawings, fields' instructions etc. are Phase 2.
                b"drawing" | b"pict" | b"instrText" | b"delText" | b"object" | b"AlternateContent" => {
                    r.skip_element()?;
                }
                _ => {}
            },
            Event::Empty(e) => match e.local_name().as_ref().as_bytes() {
                b"tab" => run.content.push(RunContent::Tab), // §17.3.3.32
                // §17.3.3.1 br
                b"br" => run.content.push(match attr(&e, b"type").as_deref() {
                    Some("page") => RunContent::PageBreak,
                    _ => RunContent::LineBreak,
                }),
                b"cr" => run.content.push(RunContent::LineBreak), // §17.3.3.4
                _ => {}
            },
            Event::End(_) if r.depth() == end => return Ok(run),
            Event::Eof => return Err(r.err("unexpected EOF in w:r").into()),
            _ => {}
        }
    }
}

fn read_text(r: &mut XmlReader<'_>) -> Result<String> {
    let end = r.depth() - 1;
    let mut s = String::new();
    loop {
        match r.next()? {
            Event::Text(t) => s.push_str(&unescape(t.as_ref())),
            Event::GeneralRef(g) => s.push_str(&unescape(&format!("&{};", <_ as AsRef<str>>::as_ref(&g)))),
            Event::CData(t) => s.push_str(t.as_ref()),
            Event::End(_) if r.depth() == end => return Ok(s),
            Event::Eof => return Err(r.err("unexpected EOF in w:t").into()),
            _ => {}
        }
    }
}

/// ECMA-376 Part 1, §17.6.17 sectPr — page size (§17.6.13) and margins (§17.6.11).
fn read_sect_pr(r: &mut XmlReader<'_>) -> Result<PageSetup> {
    let end = r.depth() - 1;
    let mut s = PageSetup::default();
    loop {
        let ev = r.next()?;
        let e = match &ev {
            Event::Start(e) | Event::Empty(e) => e.clone(),
            Event::End(_) if r.depth() == end => return Ok(s),
            Event::Eof => return Err(r.err("unexpected EOF in w:sectPr").into()),
            _ => continue,
        };
        match e.local_name().as_ref().as_bytes() {
            b"pgSz" => {
                s.width = twips(&e, b"w").filter(|t| t.0 > 0).unwrap_or(s.width);
                s.height = twips(&e, b"h").filter(|t| t.0 > 0).unwrap_or(s.height);
            }
            b"pgMar" => {
                s.margin_top = twips(&e, b"top").map(|t| Twips(t.0.abs())).unwrap_or(s.margin_top);
                s.margin_bottom = twips(&e, b"bottom").map(|t| Twips(t.0.abs())).unwrap_or(s.margin_bottom);
                s.margin_left = twips(&e, b"left").unwrap_or(s.margin_left);
                s.margin_right = twips(&e, b"right").unwrap_or(s.margin_right);
            }
            _ => {}
        }
    }
}
