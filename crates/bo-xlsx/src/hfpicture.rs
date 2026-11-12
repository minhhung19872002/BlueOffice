//! Header / footer pictures (`&G`) of a worksheet.
//!
//! A picture printed in a header or footer section is a VML shape in the drawing the
//! worksheet names with `legacyDrawingHF` (ECMA-376 Part 1, §18.3.1.50 `legacyDrawingHF`;
//! VML, ECMA-376 Part 4 §19.1.2.19 `v:shape`, §19.1.2.11 `v:imagedata`). The shape id names
//! the section: `LH` / `CH` / `RH` (left / centre / right header), `LF` / `CF` / `RF`
//! (footers), with the suffix `FIRST` or `EVEN` for the first-page and even-page variants
//! (§18.3.1.46 `headerFooter@differentFirst` / `differentOddEven`). Its `style` gives the
//! printed size and `v:imagedata/@o:relid` the image through the drawing's relationships.
//!
//! [`pictures`] reads them from the parts kept with the workbook; [`set_picture`] adds,
//! replaces or removes one, creating the drawing, its relationships part, the image part,
//! the worksheet relationship and the `legacyDrawingHF` element when needed. Everything is
//! kept in the workbook's opaque parts, so the writer saves it like any other kept part.

use std::fmt::Write as _;

use bo_model::sheet::{OpaquePart, OpaqueRel, RawXml, Workbook, Worksheet};
use bo_opc::xml::escape;
use bo_opc::{rels_part_name, resolve_target, Relationship, TargetMode};

use crate::formctl::{attr_of, first_free_block, tags, TagKind, CT_VML, REL_VML};

/// Relationship type of an image (ECMA-376 Part 1, §15.2.14).
const REL_IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
/// Content type of a relationships part (ECMA-376 Part 2, §9.3).
const CT_RELS: &str = "application/vnd.openxmlformats-package.relationships+xml";

/// The section ids of header / footer pictures (VML shape `id`).
pub const SECTIONS: [&str; 18] = [
    "LH", "CH", "RH", "LF", "CF", "RF", "LHFIRST", "CHFIRST", "RHFIRST", "LFFIRST", "CFFIRST",
    "RFFIRST", "LHEVEN", "CHEVEN", "RHEVEN", "LFEVEN", "CFEVEN", "RFEVEN",
];

/// A picture of a header / footer section.
#[derive(Debug, Clone, PartialEq)]
pub struct HfPicture {
    /// Section id (`LH`, `CF`, `RHFIRST`, …).
    pub section: String,
    /// Absolute name of the image part.
    pub image_part: String,
    /// Printed width in points.
    pub width: f64,
    /// Printed height in points.
    pub height: f64,
    /// `v:imagedata/@o:title` (the picture's name).
    pub title: String,
}

/// A CSS length of a VML `style` in points (`pt`, `px` at 96 dpi, `in`, `cm`, `mm`; a bare
/// number is pixels).
fn length_pt(v: &str) -> Option<f64> {
    let v = v.trim();
    let (num, unit) = v
        .find(|c: char| c.is_ascii_alphabetic())
        .map_or((v, ""), |i| (&v[..i], &v[i..]));
    let n: f64 = num.trim().parse().ok()?;
    let k = match unit {
        "pt" => 1.0,
        "" | "px" => 0.75,
        "in" => 72.0,
        "cm" => 72.0 / 2.54,
        "mm" => 72.0 / 25.4,
        _ => return None,
    };
    Some(n * k).filter(|x| x.is_finite() && *x > 0.0)
}

/// `width` / `height` of a VML `style` attribute (points).
fn style_size(style: &str) -> (f64, f64) {
    let mut w = None;
    let mut h = None;
    for decl in style.split(';') {
        if let Some((k, v)) = decl.split_once(':') {
            match k.trim() {
                "width" => w = length_pt(v),
                "height" => h = length_pt(v),
                _ => {}
            }
        }
    }
    (w.unwrap_or(72.0), h.unwrap_or(72.0))
}

/// One section shape of a VML drawing: (byte span, section id, style, image relid, title).
struct Shape {
    span: (usize, usize),
    section: String,
    style: String,
    relid: Option<String>,
    title: String,
}

fn shapes(xml: &str) -> Vec<Shape> {
    let ts = tags(xml);
    let mut out = Vec::new();
    let mut k = 0;
    while k < ts.len() {
        let t = &ts[k];
        if t.kind == TagKind::Close || t.local(xml) != "shape" {
            k += 1;
            continue;
        }
        let head = t.text(xml);
        let id = attr_of(head, "id").unwrap_or_default();
        // The shape's extent: up to its end tag (or the empty tag itself).
        let mut end = t.end;
        let mut j = k + 1;
        let mut relid = None;
        let mut title = String::new();
        if t.kind == TagKind::Open {
            let mut depth = 1;
            while j < ts.len() {
                let u = &ts[j];
                if u.local(xml) == "imagedata" && u.kind != TagKind::Close {
                    let h = u.text(xml);
                    relid = attr_of(h, "o:relid").or_else(|| attr_of(h, "r:id"));
                    title = attr_of(h, "o:title").unwrap_or_default();
                }
                if u.local(xml) == "shape" {
                    match u.kind {
                        TagKind::Open => depth += 1,
                        TagKind::Close => depth -= 1,
                        TagKind::Empty => {}
                    }
                    if depth == 0 {
                        end = u.end;
                        break;
                    }
                }
                j += 1;
            }
        }
        if SECTIONS.iter().any(|s| s.eq_ignore_ascii_case(&id)) {
            out.push(Shape {
                span: (t.start, end),
                section: id.to_ascii_uppercase(),
                style: attr_of(head, "style").unwrap_or_default(),
                relid,
                title,
            });
        }
        k = j.max(k + 1);
    }
    out
}

/// The `legacyDrawingHF` relationship id of a worksheet.
fn hf_rid(ws: &Worksheet) -> Option<String> {
    ws.unknown.iter().find_map(|u| {
        let ts = tags(&u.xml);
        let t = ts
            .iter()
            .find(|t| t.kind != TagKind::Close && t.local(&u.xml) == "legacyDrawingHF")?;
        attr_of(t.text(&u.xml), "r:id")
    })
}

/// Absolute name of the worksheet's header / footer drawing, if it has one.
fn hf_part(ws: &Worksheet) -> Option<String> {
    let rid = hf_rid(ws)?;
    ws.rels
        .iter()
        .find(|r| r.id == rid && !r.external)
        .map(|r| r.target.clone())
}

fn part<'a>(parts: &'a [OpaquePart], name: &str) -> Option<&'a OpaquePart> {
    parts.iter().find(|p| p.name.eq_ignore_ascii_case(name))
}

/// Relationships of a kept part (from its kept relationships part).
fn part_rels(parts: &[OpaquePart], name: &str) -> Vec<Relationship> {
    part(parts, &rels_part_name(name))
        .and_then(|p| bo_opc::parse_relationships(&p.data, &rels_part_name(name)).ok())
        .unwrap_or_default()
}

/// The header / footer pictures of a worksheet.
#[must_use]
pub fn pictures(ws: &Worksheet, parts: &[OpaquePart]) -> Vec<HfPicture> {
    let Some(vml) = hf_part(ws) else {
        return Vec::new();
    };
    let Some(data) = part(parts, &vml) else {
        return Vec::new();
    };
    let xml = String::from_utf8_lossy(&data.data);
    let rels = part_rels(parts, &vml);
    shapes(&xml)
        .into_iter()
        .filter_map(|s| {
            let rel = rels
                .iter()
                .find(|r| Some(&r.id) == s.relid.as_ref() && r.mode == TargetMode::Internal)?;
            let (width, height) = style_size(&s.style);
            Some(HfPicture {
                section: s.section,
                image_part: resolve_target(&vml, &rel.target),
                width,
                height,
                title: s.title,
            })
        })
        .collect()
}

/// A picture to put in a section.
#[derive(Debug, Clone, PartialEq)]
pub struct NewPicture<'a> {
    /// Image bytes (PNG, JPEG, GIF, BMP).
    pub data: &'a [u8],
    /// File extension of the image (`png`, `jpeg`, …).
    pub ext: &'a str,
    /// Printed width in points.
    pub width: f64,
    /// Printed height in points.
    pub height: f64,
    /// Name of the picture.
    pub title: &'a str,
}

fn image_content_type(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        _ => "image/png",
    }
}

/// A part name `prefix{n}.ext` not used yet.
fn fresh(parts: &[OpaquePart], prefix: &str, ext: &str) -> String {
    (1..=parts.len() + 1)
        .map(|n| format!("{prefix}{n}.{ext}"))
        .find(|n| part(parts, n).is_none())
        .unwrap_or_default()
}

/// A relationship id not among `ids`.
fn fresh_rid(ids: &[&str]) -> String {
    (1..=ids.len() + 1)
        .map(|n| format!("rId{n}"))
        .find(|c| !ids.contains(&c.as_str()))
        .unwrap_or_default()
}

fn put(parts: &mut Vec<OpaquePart>, name: &str, content_type: Option<&str>, data: Vec<u8>) {
    if let Some(p) = parts.iter_mut().find(|p| p.name.eq_ignore_ascii_case(name)) {
        p.data = data;
        if content_type.is_some() {
            p.content_type = content_type.map(str::to_owned);
        }
    } else {
        parts.push(OpaquePart {
            name: name.to_owned(),
            content_type: content_type.map(str::to_owned),
            data,
        });
    }
}

/// An empty header / footer drawing with shape ids in block `block` and the picture-frame
/// shape type (`o:spt="75"`).
fn empty_vml(block: u32) -> String {
    format!(
        "<xml xmlns:v=\"urn:schemas-microsoft-com:vml\" xmlns:o=\"urn:schemas-microsoft-com:office:office\" xmlns:x=\"urn:schemas-microsoft-com:office:excel\"><o:shapelayout v:ext=\"edit\"><o:idmap v:ext=\"edit\" data=\"{block}\"/></o:shapelayout><v:shapetype id=\"_x0000_t75\" coordsize=\"21600,21600\" o:spt=\"75\" o:preferrelative=\"t\" path=\"m,l,21600r21600,l21600,xe\" filled=\"f\" stroked=\"f\"><v:stroke joinstyle=\"miter\"/><v:path o:extrusionok=\"f\" gradientshapeok=\"t\" o:connecttype=\"rect\"/><o:lock v:ext=\"edit\" aspectratio=\"t\"/></v:shapetype></xml>"
    )
}

/// Adds or replaces (`Some`) or removes (`None`) the picture of section `section` of sheet
/// `sheet`. `Err` for an unknown section or sheet.
///
/// # Errors
/// A message when the section id or the sheet does not exist.
pub fn set_picture(
    wb: &mut Workbook,
    sheet: usize,
    section: &str,
    picture: Option<NewPicture<'_>>,
) -> Result<(), String> {
    let section = section.to_ascii_uppercase();
    if !SECTIONS.contains(&section.as_str()) {
        return Err(format!("unknown header / footer section {section}"));
    }
    let ws = wb.sheets.get(sheet).ok_or("no such sheet")?;
    let existing = hf_part(ws).filter(|p| part(&wb.opaque.parts, p).is_some());
    if existing.is_none() && picture.is_none() {
        return Ok(());
    }
    let vml = if let Some(p) = existing {
        p
    } else {
        let block = first_free_block(&wb.opaque.parts);
        let name = fresh(&wb.opaque.parts, "/xl/drawings/vmlDrawingHF", "vml");
        put(
            &mut wb.opaque.parts,
            &name,
            Some(CT_VML),
            empty_vml(block).into_bytes(),
        );
        let ws = &mut wb.sheets[sheet];
        let ids: Vec<&str> = ws.rels.iter().map(|r| r.id.as_str()).collect();
        let rid = fresh_rid(&ids);
        ws.rels.push(OpaqueRel {
            id: rid.clone(),
            rel_type: REL_VML.to_owned(),
            target: name.clone(),
            external: false,
        });
        ws.unknown.retain(|u| !u.xml.contains("legacyDrawingHF"));
        ws.unknown.push(RawXml {
            after: "legacyDrawingHF".into(),
            xml: format!("<legacyDrawingHF r:id=\"{rid}\"/>"),
        });
        name
    };
    let mut xml = part(&wb.opaque.parts, &vml)
        .map(|p| String::from_utf8_lossy(&p.data).into_owned())
        .unwrap_or_default();
    let mut rels = part_rels(&wb.opaque.parts, &vml);
    // Drop the section's current shape and its image relationship.
    let mut dropped_images = Vec::new();
    if let Some(s) = shapes(&xml).into_iter().find(|s| s.section == section) {
        xml.replace_range(s.span.0..s.span.1, "");
        if let Some(id) = &s.relid {
            let still_used = shapes(&xml).iter().any(|o| o.relid.as_ref() == Some(id));
            if !still_used {
                if let Some(k) = rels.iter().position(|r| &r.id == id) {
                    dropped_images.push(resolve_target(&vml, &rels[k].target));
                    rels.remove(k);
                }
            }
        }
    }
    if let Some(p) = picture {
        let ext = if p.ext.eq_ignore_ascii_case("jpg") {
            "jpeg".to_owned()
        } else {
            p.ext.to_ascii_lowercase()
        };
        let img = fresh(&wb.opaque.parts, "/xl/media/hfimage", &ext);
        put(
            &mut wb.opaque.parts,
            &img,
            Some(image_content_type(&ext)),
            p.data.to_vec(),
        );
        let ids: Vec<&str> = rels.iter().map(|r| r.id.as_str()).collect();
        let rid = fresh_rid(&ids);
        rels.push(Relationship {
            id: rid.clone(),
            rel_type: REL_IMAGE.to_owned(),
            target: crate::write::relative_target(&vml, &img),
            mode: TargetMode::Internal,
        });
        // Shape number: one past the largest `_x0000_s` of the drawing, in its block.
        let block = tags(&xml)
            .iter()
            .find(|t| t.local(&xml) == "idmap")
            .and_then(|t| attr_of(t.text(&xml), "data"))
            .and_then(|d| {
                d.split(',')
                    .next()
                    .and_then(|n| n.trim().parse::<u32>().ok())
            })
            .unwrap_or(1);
        let used = xml
            .match_indices("_x0000_s")
            .filter_map(|(i, m)| {
                let rest = &xml[i + m.len()..];
                let n: String = rest.chars().take_while(char::is_ascii_digit).collect();
                n.parse::<u32>().ok()
            })
            .max()
            .unwrap_or(block * 1024);
        let spid = used.max(block * 1024) + 1;
        let z = shapes(&xml).len() + 1;
        let mut shape = String::new();
        let _ = write!(
            shape,
            "<v:shape id=\"{section}\" o:spid=\"_x0000_s{spid}\" type=\"#_x0000_t75\" style=\"position:absolute;margin-left:0;margin-top:0;width:{:.2}pt;height:{:.2}pt;z-index:{z}\"><v:imagedata o:relid=\"{rid}\" o:title=\"{}\"/><o:lock v:ext=\"edit\" rotation=\"t\"/></v:shape>",
            p.width.max(1.0),
            p.height.max(1.0),
            escape(p.title)
        );
        let at = xml.rfind("</xml>").unwrap_or(xml.len());
        xml.insert_str(at, &shape);
    }
    let rels_name = rels_part_name(&vml);
    if shapes(&xml).is_empty() {
        // No picture left: the drawing, its relationships and the sheet's reference go.
        wb.opaque.parts.retain(|p| {
            !p.name.eq_ignore_ascii_case(&vml) && !p.name.eq_ignore_ascii_case(&rels_name)
        });
        let ws = &mut wb.sheets[sheet];
        if let Some(rid) = hf_rid(ws) {
            ws.rels.retain(|r| r.id != rid);
        }
        ws.unknown.retain(|u| !u.xml.contains("legacyDrawingHF"));
    } else {
        put(&mut wb.opaque.parts, &vml, Some(CT_VML), xml.into_bytes());
        put(
            &mut wb.opaque.parts,
            &rels_name,
            Some(CT_RELS),
            crate::write::bo_opc_rels_xml(&rels).into_bytes(),
        );
    }
    // An image no relationships part names any more is removed.
    for img in dropped_images {
        let file = img.rsplit('/').next().unwrap_or(&img).to_owned();
        let referenced =
            wb.opaque.parts.iter().any(|p| {
                p.name.ends_with(".rels") && String::from_utf8_lossy(&p.data).contains(&file)
            });
        if !referenced {
            wb.opaque
                .parts
                .retain(|p| !p.name.eq_ignore_ascii_case(&img));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_and_styles() {
        assert_eq!(length_pt("30pt"), Some(30.0));
        assert_eq!(length_pt("40px"), Some(30.0));
        assert_eq!(length_pt("1in"), Some(72.0));
        assert!((length_pt("2.54cm").unwrap_or(0.0) - 72.0).abs() < 1e-9);
        assert_eq!(length_pt("x"), None);
        assert_eq!(
            style_size("position:absolute;margin-left:0;width:102pt;height:30pt;z-index:1"),
            (102.0, 30.0)
        );
    }

    #[test]
    fn scans_section_shapes() {
        let xml = "<xml><v:shape id=\"CH\" o:spid=\"_x0000_s1025\" style=\"width:20pt;height:10pt\"><v:imagedata o:relid=\"rId1\" o:title=\"logo\"/></v:shape><v:shape id=\"_x0000_s9\"/><v:shape id=\"rffirst\"><v:imagedata o:relid=\"rId2\"/></v:shape></xml>";
        let s = shapes(xml);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].section, "CH");
        assert_eq!(s[0].relid.as_deref(), Some("rId1"));
        assert_eq!(s[0].title, "logo");
        assert_eq!(s[1].section, "RFFIRST");
    }
}
