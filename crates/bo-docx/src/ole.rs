//! Embedded and linked objects in WordprocessingML (CORE-EMBED-001).
//!
//! `w:object` (ECMA-376 Part 1, §17.3.3.19 object; `w:dxaOrig` / `w:dyaOrig` original
//! size in twips) holds a VML shape (`v:shape`, ECMA-376 Part 4, §19.1.2.19) whose
//! `v:imagedata` (§19.1.2.11) is the preview picture, and the Office VML
//! `o:OLEObject` (`CT_OLEObject`: `Type` Embed | Link, `ProgID`, `ShapeID`,
//! `DrawAspect`, `ObjectID`, `r:id`, `UpdateMode`; children `o:LinkType`,
//! `o:LockedField`, `o:FieldCodes`). A linked object sits in the result of a
//! §17.16.5.32 LINK field whose instruction names the source file and item.
//!
//! Reading turns the object into a picture ([`Image`]) carrying [`OleObject`], keeping
//! the source XML: an object that is not changed is written back byte-for-byte, its
//! package part with it. Nothing in the object is ever run.

use crate::shape::{css_len, find_local};
use bo_model::ole::{OleLink, OleObject};
use bo_model::slide::XmlElement;
use bo_model::{fingerprint, Emu, Image};
use bo_opc::xml::escape;

/// Office VML namespace (`o:`).
pub(crate) const O_NS: &str = "urn:schemas-microsoft-com:office:office";
/// VML namespace (`v:`).
pub(crate) const V_NS: &str = "urn:schemas-microsoft-com:vml";

/// What the reader found in a `w:object`.
pub(crate) struct ReadObject {
    /// The object.
    pub ole: OleObject,
    /// Shown size (EMU).
    pub width: Emu,
    /// Shown size (EMU).
    pub height: Emu,
    /// Relationship id of the preview picture (`v:imagedata/@r:id`).
    pub preview_rid: String,
}

/// Attribute by local name whatever its prefix.
fn attr_any<'a>(el: &'a XmlElement, local: &str) -> Option<&'a str> {
    el.attrs
        .iter()
        .find(|(k, _)| k.rsplit(':').next() == Some(local))
        .map(|(_, v)| v.as_str())
}

/// Read a `w:object` element (`xml`, from part `part`). `rel` resolves a relationship
/// id to (target part name or external URI, external?). `None` when the object has
/// no preview picture or no `o:OLEObject` (it then stays verbatim, unrendered).
pub(crate) fn read(
    xml: &str,
    part: &str,
    rel: &dyn Fn(&str) -> Option<(String, bool)>,
) -> Option<ReadObject> {
    let dom = bo_drawingml::dom::parse(xml.as_bytes(), part).ok()?;
    let ole_el = find_local(&dom, "OLEObject")?;
    let shape = dom
        .elements()
        .find(|e| matches!(e.local(), "shape" | "rect"))?;
    let imagedata = find_local(shape, "imagedata")?;
    let preview_rid = attr_any(imagedata, "id")
        .or_else(|| attr_any(imagedata, "relid"))?
        .to_owned();
    // §19.1.2.19 style: width / height of the shape.
    let style = shape.attr("style").unwrap_or_default();
    let get = |k: &str| {
        style.split(';').find_map(|kv| {
            let (n, v) = kv.split_once(':')?;
            (n.trim().eq_ignore_ascii_case(k)).then(|| css_len(v))?
        })
    };
    // §17.3.3.19 dxaOrig / dyaOrig (twips) when the style has no size.
    let orig = |k: &str| {
        dom.attrs
            .iter()
            .find(|(n, _)| n.rsplit(':').next() == Some(k))
            .and_then(|(_, v)| v.parse::<i64>().ok())
            .map(|tw| tw * 635)
    };
    let width = get("width").or_else(|| orig("dxaOrig")).unwrap_or(0);
    let height = get("height").or_else(|| orig("dyaOrig")).unwrap_or(0);
    if width <= 0 || height <= 0 {
        return None;
    }
    let kind = ole_el.attr("Type").unwrap_or("Embed");
    let target = attr_any(ole_el, "id").and_then(rel);
    let mut ole = OleObject {
        prog_id: ole_el.attr("ProgID").unwrap_or_default().to_owned(),
        icon: ole_el.attr("DrawAspect") == Some("Icon"),
        ..OleObject::default()
    };
    if kind == "Link" {
        ole.link = Some(OleLink {
            source: target.map(|(t, _)| t).unwrap_or_default(),
            item: String::new(),
            auto_update: ole_el.attr("UpdateMode") == Some("Always"),
        });
    } else {
        ole.package = target.filter(|(_, ext)| !ext).map(|(t, _)| t)?;
    }
    Some(ReadObject {
        ole,
        width: Emu(width),
        height: Emu(height),
        preview_rid,
    })
}

/// Fingerprint the writer compares to decide whether the source XML still holds.
pub(crate) fn object_fingerprint(img: &Image) -> u64 {
    fingerprint(&(&img.ole, img.width, img.height, &img.media))
}

/// Points with at most two decimals.
fn pt(emu: i64) -> String {
    let v = ((emu as f64 / 12_700.0) * 100.0).round() / 100.0;
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// A new `w:object` for `img` (its [`Image::ole`]): `image_rid` is the preview picture's
/// relationship, `object_rid` the embedded part's (or the linked file's) relationship,
/// `n` the writer's object counter (mixed into the VML shape id).
pub(crate) fn object_xml(img: &Image, image_rid: &str, object_rid: &str, n: u32) -> String {
    let Some(ole) = img.ole.as_deref() else {
        return String::new();
    };
    // Ids unique in the document also next to objects kept verbatim (which may come
    // from an earlier save): mixed from the object's content and the write counter.
    let n = (object_fingerprint(img) ^ u64::from(n)) % 900_000_000 + 100_000;
    let shape_id = format!("_x0000_i{n}");
    let mut x = format!(
        // §17.3.3.19 object; dxaOrig / dyaOrig in twips (1 twip = 635 EMU).
        "<w:object w:dxaOrig=\"{}\" w:dyaOrig=\"{}\"><v:shape xmlns:v=\"{V_NS}\" xmlns:o=\"{O_NS}\" id=\"{shape_id}\" style=\"width:{}pt;height:{}pt\" o:ole=\"\"><v:imagedata r:id=\"{image_rid}\" o:title=\"\"/></v:shape>",
        (img.width.0 / 635).max(1),
        (img.height.0 / 635).max(1),
        pt(img.width.0),
        pt(img.height.0),
    );
    let aspect = if ole.icon { "Icon" } else { "Content" };
    match &ole.link {
        Some(l) => {
            let mode = if l.auto_update { "Always" } else { "OnCall" };
            let _ = std::fmt::Write::write_fmt(
                &mut x,
                format_args!(
                    "<o:OLEObject xmlns:o=\"{O_NS}\" Type=\"Link\" ProgID=\"{}\" ShapeID=\"{shape_id}\" DrawAspect=\"{aspect}\" r:id=\"{object_rid}\" UpdateMode=\"{mode}\"><o:LinkType>Picture</o:LinkType><o:LockedField>false</o:LockedField></o:OLEObject>",
                    escape(&ole.prog_id)
                ),
            );
        }
        None => {
            let _ = std::fmt::Write::write_fmt(
                &mut x,
                format_args!(
                    "<o:OLEObject xmlns:o=\"{O_NS}\" Type=\"Embed\" ProgID=\"{}\" ShapeID=\"{shape_id}\" DrawAspect=\"{aspect}\" ObjectID=\"_{}\" r:id=\"{object_rid}\"/>",
                    escape(&ole.prog_id),
                    n
                ),
            );
        }
    }
    x.push_str("</w:object>");
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    const OBJ: &str = r##"<w:object xmlns:w="w" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office" xmlns:r="r" w:dxaOrig="2880" w:dyaOrig="1440"><v:shape id="_x0000_i1025" type="#_x0000_t75" style="width:144pt;height:72pt" o:ole=""><v:imagedata r:id="rId8" o:title=""/></v:shape><o:OLEObject Type="Embed" ProgID="Excel.Sheet.12" ShapeID="_x0000_i1025" DrawAspect="Content" ObjectID="_1" r:id="rId9"/></w:object>"##;

    #[test]
    fn reads_embedded_object() {
        let rel = |id: &str| match id {
            "rId9" => Some(("/word/embeddings/Book1.xlsx".to_owned(), false)),
            _ => None,
        };
        let o = read(OBJ, "/word/document.xml", &rel).unwrap();
        // 144 pt × 72 pt = 1 828 800 × 914 400 EMU.
        assert_eq!((o.width, o.height), (Emu(1_828_800), Emu(914_400)));
        assert_eq!(o.preview_rid, "rId8");
        assert_eq!(o.ole.prog_id, "Excel.Sheet.12");
        assert_eq!(o.ole.package, "/word/embeddings/Book1.xlsx");
        assert!(o.ole.link.is_none() && !o.ole.icon);
    }

    #[test]
    fn reads_linked_object_and_size_from_twips() {
        let xml = OBJ
            .replace("style=\"width:144pt;height:72pt\"", "")
            .replace("Type=\"Embed\"", "Type=\"Link\" UpdateMode=\"Always\"");
        let rel = |_: &str| Some((r"file:///C:\b.xlsx".to_owned(), true));
        let o = read(&xml, "/word/document.xml", &rel).unwrap();
        // 2880 × 1440 twips = 2 inches × 1 inch.
        assert_eq!((o.width, o.height), (Emu(1_828_800), Emu(914_400)));
        let l = o.ole.link.unwrap();
        assert!(l.auto_update);
        assert_eq!(l.source, r"file:///C:\b.xlsx");
    }

    #[test]
    fn object_without_preview_stays_opaque() {
        let xml = r#"<w:object xmlns:w="w"><v:shape xmlns:v="v" style="width:72pt;height:72pt"/></w:object>"#;
        assert!(read(xml, "/word/document.xml", &|_| None).is_none());
    }
}
