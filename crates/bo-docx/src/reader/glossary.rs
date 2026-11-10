//! Glossary document part → [`Glossary`] (ECMA-376 Part 1, §17.12; part §11.3.8).
//!
//! The part is a document of its own: `w:glossaryDocument` / `w:docParts` /
//! `w:docPart`, each with `w:docPartPr` and a `w:docPartBody` of ordinary block
//! content, resolved against the part's own relationships, styles and numbering.
//! Every `w:docPart` keeps its source XML so untouched entries are written back
//! byte for byte.

use super::{children, name, read_blocks, read_numbering, read_styles, Ctx, REL_NUMBERING};
use crate::Result;
use bo_model::glossary::{DocPart, DocPartProps, Glossary, GlossarySource};
use bo_model::{fingerprint, Document, Raw};
use bo_opc::xml::{attr, XmlReader};
use bo_opc::{Package, REL_STYLES};
use quick_xml::events::Event;
use std::collections::BTreeMap;
use std::sync::Arc;

/// ECMA-376 Part 1, §11.3.8 Glossary Document part relationship.
pub(crate) const REL_GLOSSARY: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/glossaryDocument";

/// Read the glossary document part `part` of `pkg`; its pictures go to `media`.
pub(super) fn read_glossary(
    pkg: &Package,
    part: &str,
    theme: Option<&Arc<bo_model::slide::Theme>>,
    media: &mut BTreeMap<String, Arc<Vec<u8>>>,
) -> Result<Option<Glossary>> {
    let Some(data) = pkg.part(part).map(|p| &p.data) else {
        return Ok(None);
    };
    let mut ctx = Ctx::new(pkg, part)?;
    ctx.theme = theme.cloned();
    // The glossary's own styles and numbering (read through a scratch document).
    let mut scratch = Document::default();
    if let Some(p) = pkg
        .related_part(part, REL_STYLES)?
        .and_then(|n| pkg.part(&n))
    {
        read_styles(&p.data, &p.name, &mut scratch)?;
    }
    if let Some(p) = pkg
        .related_part(part, REL_NUMBERING)?
        .and_then(|n| pkg.part(&n))
    {
        scratch.numbering = read_numbering(&p.data, &p.name)?;
    }
    let mut g = Glossary {
        source: Some(GlossarySource {
            part: part.to_owned(),
            styles_fingerprint: super::styles_fingerprint(&scratch),
            numbering_fingerprint: fingerprint(&scratch.numbering),
            ..GlossarySource::default()
        }),
        styles: scratch.styles,
        default_run: scratch.default_run,
        default_para: scratch.default_para,
        numbering: scratch.numbering,
        parts: Vec::new(),
    };
    let mut r = XmlReader::new(data, part);
    let (mut prefix, mut suffix) = (String::new(), String::new());
    loop {
        match r.read()? {
            // §17.12.8 docParts
            Event::Start(e) if name(&e) == b"docParts" => {
                prefix = r.slice(0, r.pos());
                let end = r.depth() - 1;
                loop {
                    let before = r.pos();
                    match r.read()? {
                        Event::Start(e) if name(&e) == b"docPart" => {
                            let mut part = read_doc_part(&mut r, &ctx, media)?;
                            part.src = Some(Raw::new(
                                r.slice(before, r.pos()),
                                part.content_fingerprint(),
                            ));
                            g.parts.push(part);
                        }
                        Event::Start(_) => r.skip_element()?,
                        Event::End(_) if r.depth() == end => {
                            suffix = r.slice(before, data.len());
                            break;
                        }
                        Event::Eof => return Err(r.err("unexpected end of docParts").into()),
                        _ => {}
                    }
                }
                break;
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if let Some(s) = g.source.as_mut() {
        s.prefix = prefix;
        s.suffix = suffix;
        s.count = g.parts.len();
    }
    Ok(Some(g))
}

/// §17.12.5 docPart (its start tag was just read).
fn read_doc_part(
    r: &mut XmlReader<'_>,
    ctx: &Ctx<'_>,
    media: &mut BTreeMap<String, Arc<Vec<u8>>>,
) -> Result<DocPart> {
    let mut part = DocPart::default();
    let end = r.depth() - 1;
    loop {
        match r.read()? {
            Event::Start(e) => match name(&e).as_slice() {
                b"docPartPr" => part.props = read_props(r)?,
                // §17.12.6 docPartBody: block content (CT_Body).
                b"docPartBody" => part.blocks = read_blocks(r, ctx, media, &mut None)?,
                _ => r.skip_element()?,
            },
            Event::End(_) if r.depth() == end => return Ok(part),
            Event::Eof => return Err(r.err("unexpected end of docPart").into()),
            _ => {}
        }
    }
}

/// §17.12.7 docPartPr.
fn read_props(r: &mut XmlReader<'_>) -> Result<DocPartProps> {
    let mut p = DocPartProps::default();
    children(r, |r, e, s, _| {
        match name(e).as_slice() {
            // §17.12.13 name
            b"name" => p.name = attr(e, b"val").unwrap_or_default(),
            // §17.12.14 style
            b"style" => p.style = attr(e, b"val"),
            // §17.12.4 description
            b"description" => p.description = attr(e, b"val").unwrap_or_default(),
            // §17.12.11 guid
            b"guid" => p.guid = attr(e, b"val"),
            // §17.12.3 category: name (§17.12.12) and gallery (§17.12.9).
            b"category" if s => {
                children(r, |_, e, s, _| {
                    match name(e).as_slice() {
                        b"name" => p.category = attr(e, b"val").unwrap_or_default(),
                        b"gallery" => p.gallery = attr(e, b"val").unwrap_or_default(),
                        _ => {}
                    }
                    Ok(!s)
                })?;
                return Ok(true);
            }
            // §17.12.15 types / §17.12.16 type
            b"types" if s => {
                children(r, |_, e, s, _| {
                    if name(e) == b"type" {
                        p.types.extend(attr(e, b"val"));
                    }
                    Ok(!s)
                })?;
                return Ok(true);
            }
            // §17.12.2 behaviors / §17.12.1 behavior
            b"behaviors" if s => {
                children(r, |_, e, s, _| {
                    if name(e) == b"behavior" {
                        p.behaviors.extend(attr(e, b"val"));
                    }
                    Ok(!s)
                })?;
                return Ok(true);
            }
            _ => {}
        }
        Ok(!s)
    })?;
    Ok(p)
}
