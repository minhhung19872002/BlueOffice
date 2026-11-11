//! SmartArt in WordprocessingML (CORE-SMART-005). A diagram is a `w:drawing` (ECMA-376 Part 1,
//! §17.3.3.9) whose `a:graphicData` (§20.1.2.2.17) holds `dgm:relIds` (§21.4.2.6): the data model,
//! layout, quick style and colours parts are relationships of the part holding the drawing, and
//! so is the drawing part ([MS-ODRAWXML] `dsp:drawing`) the data model names.
//!
//! In the document model a diagram is an [`Image`] whose media is its drawing part (or, without
//! one, its data model part). Diagrams read from a file keep their source XML and parts
//! byte-for-byte; a diagram BlueOffice creates or edits has its five parts in
//! [`Document::media`] under the names of [`bo_model::diagram::part_names`] and no source XML,
//! and the writer gives it a new frame and relationships.

use bo_model::diagram::{self, DiagramPart};
use bo_model::{Document, Image};
use bo_opc::{Limits, Package};
use std::sync::Arc;

/// Data model and definitions of a diagram with their part names, and its pictures.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiagramSource {
    /// Data model part (§21.4.3) and its bytes.
    pub data: (String, Vec<u8>),
    /// Layout definition part (§21.4.2.15), if any.
    pub layout: Option<(String, Vec<u8>)>,
    /// Quick style part (§21.4.4 styleDef), if any.
    pub style: Option<(String, Vec<u8>)>,
    /// Colours part (§21.4.4 colorsDef), if any.
    pub colors: Option<(String, Vec<u8>)>,
    /// Image relationships of the data model part: (id, image part name) — the pictures of the
    /// nodes (`dgm:pt/dgm:spPr` picture fills).
    pub pictures: Vec<(String, String)>,
    /// Image relationships of the drawing part (the frame's media, when it is a drawing part).
    pub drawing_pictures: Vec<(String, String)>,
    /// The image parts either list names, with their bytes.
    pub images: Vec<(String, Arc<Vec<u8>>)>,
}

/// Part name of the relationships of part `name` (ECMA-376 Part 2: `<dir>/_rels/<file>.rels`).
#[must_use]
pub fn rels_name(name: &str) -> String {
    bo_opc::rels_part_name(name)
}

/// Image relationships (id, absolute part name) in the relationships part `rels` of `part`.
fn image_rels(rels: &[u8], part: &str) -> Vec<(String, String)> {
    bo_opc::parse_relationships(rels, &rels_name(part))
        .unwrap_or_default()
        .into_iter()
        .filter(|r| r.rel_type.ends_with("/image") && r.mode == bo_opc::TargetMode::Internal)
        .map(|r| (r.id, bo_opc::resolve_target(part, &r.target)))
        .collect()
}

/// Is `img` a SmartArt frame (its media a diagram drawing or data model part)?
#[must_use]
pub fn is_diagram(doc: &Document, img: &Image) -> bool {
    img.shape.is_none()
        && !diagram::is_group_name(&img.media)
        && diagram::split_name(&img.media)
            .is_some_and(|(k, _, _)| matches!(k, DiagramPart::Drawing | DiagramPart::Data))
        && doc
            .media
            .get(&img.media)
            .is_some_and(|b| diagram::is_diagram_xml(b))
}

/// The parts of a diagram BlueOffice wrote into [`Document::media`] (new or edited diagrams).
#[must_use]
pub fn own_parts(doc: &Document, img: &Image) -> Option<[String; 5]> {
    if img.src.is_some() || diagram::is_group_name(&img.media) {
        return None;
    }
    let names = DiagramPart::ALL.map(|k| diagram::sibling(&img.media, k).unwrap_or_default());
    names
        .iter()
        .all(|n| doc.media.contains_key(n))
        .then_some(names)
}

/// `r:dm` and `r:lo` / `r:qs` / `r:cs` relationship ids of the `dgm:relIds` in a drawing's XML
/// (§21.4.2.6).
fn rel_ids(xml: &str) -> Option<(String, [Option<String>; 3])> {
    let at = xml.find(":relIds")?;
    let tag = &xml[at..at + xml[at..].find('>')?];
    let val = |name: &str| {
        let key = format!(":{name}=\"");
        let s = tag.find(&key)? + key.len();
        Some(tag[s..s + tag[s..].find('"')?].to_owned())
    };
    Some((val("dm")?, [val("lo"), val("qs"), val("cs")]))
}

/// Data model and layout of SmartArt `img`: from the document's media for diagrams BlueOffice
/// wrote, else through the main document part's relationships in the source package.
#[must_use]
pub fn diagram_source(doc: &Document, img: &Image) -> Option<DiagramSource> {
    diagram_source_in(doc, img, None)
}

/// The source package of the document, read once for several [`diagram_source_in`] calls.
#[must_use]
pub fn source_package(doc: &Document) -> Option<Package> {
    Package::read(&doc.source.as_ref()?.bytes, &Limits::default()).ok()
}

/// [`diagram_source`] reading a file diagram's parts from `pkg` (the document's source package,
/// [`source_package`]) instead of opening the package again.
#[must_use]
pub fn diagram_source_in(
    doc: &Document,
    img: &Image,
    pkg: Option<&Package>,
) -> Option<DiagramSource> {
    let get = |k: DiagramPart| {
        let n = diagram::sibling(&img.media, k)?;
        let b = doc.media.get(&n)?;
        Some((n, b.as_ref().clone()))
    };
    let is_drawing =
        diagram::split_name(&img.media).is_some_and(|(k, _, _)| k == DiagramPart::Drawing);
    if img.src.is_none() {
        if let Some(data) = get(DiagramPart::Data) {
            // The parts BlueOffice wrote: relationships and images are media of the document.
            let pics = |part: &str| {
                doc.media
                    .get(&rels_name(part))
                    .map(|r| image_rels(r, part))
                    .unwrap_or_default()
            };
            let pictures = pics(&data.0);
            let drawing_pictures = if is_drawing {
                pics(&img.media)
            } else {
                Vec::new()
            };
            let images = pictures
                .iter()
                .chain(&drawing_pictures)
                .filter_map(|(_, n)| Some((n.clone(), doc.media.get(n)?.clone())))
                .collect();
            return Some(DiagramSource {
                layout: get(DiagramPart::Layout),
                style: get(DiagramPart::Style),
                colors: get(DiagramPart::Colors),
                data,
                pictures,
                drawing_pictures,
                images,
            });
        }
    }
    let src = doc.source.as_ref()?;
    let (dm, [lo, qs, cs]) = rel_ids(&img.src.as_ref()?.xml)?;
    let owned;
    let pkg = if let Some(p) = pkg {
        p
    } else {
        owned = Package::read(&src.bytes, &Limits::default()).ok()?;
        &owned
    };
    let rels = pkg.relationships(&src.main_part).ok()?;
    let part = |id: &str| {
        let r = rels.iter().find(|r| r.id == id)?;
        let name = bo_opc::resolve_target(&src.main_part, &r.target);
        let p = pkg.part(&name)?;
        Some((name, p.data.clone()))
    };
    let pics = |part: &str| {
        pkg.part(&rels_name(part))
            .map(|r| image_rels(&r.data, part))
            .unwrap_or_default()
    };
    let data = part(&dm)?;
    let pictures = pics(&data.0);
    let drawing_pictures = if is_drawing {
        pics(&img.media)
    } else {
        Vec::new()
    };
    let images = pictures
        .iter()
        .chain(&drawing_pictures)
        .filter_map(|(_, n)| {
            let bytes = match doc.media.get(n) {
                Some(b) => b.clone(),
                None => Arc::new(pkg.part(n)?.data.clone()),
            };
            Some((n.clone(), bytes))
        })
        .collect();
    Some(DiagramSource {
        layout: lo.as_deref().and_then(part),
        style: qs.as_deref().and_then(part),
        colors: cs.as_deref().and_then(part),
        data,
        pictures,
        drawing_pictures,
        images,
    })
}

/// The relationships parts (data model, drawing) of the diagram parts BlueOffice wrote for `img`
/// that are media of the document.
#[must_use]
pub fn own_rels(doc: &Document, img: &Image) -> Vec<String> {
    [DiagramPart::Data, DiagramPart::Drawing]
        .iter()
        .filter_map(|k| diagram::sibling(&img.media, *k))
        .map(|n| rels_name(&n))
        .filter(|n| doc.media.contains_key(n))
        .collect()
}

/// Every part name of the source package (new parts must not take one).
#[must_use]
pub fn source_part_names(doc: &Document) -> Vec<String> {
    let Some(pkg) = doc
        .source
        .as_ref()
        .and_then(|s| Package::read(&s.bytes, &Limits::default()).ok())
    else {
        return Vec::new();
    };
    pkg.parts().iter().map(|p| p.name.clone()).collect()
}

/// Diagram part names of the source package (`/word/diagrams/…`), so new parts do not take them.
#[must_use]
pub fn source_diagram_parts(doc: &Document) -> Vec<String> {
    let Some(pkg) = doc
        .source
        .as_ref()
        .and_then(|s| Package::read(&s.bytes, &Limits::default()).ok())
    else {
        return Vec::new();
    };
    pkg.parts()
        .iter()
        .filter(|p| p.name.contains("/diagrams/"))
        .map(|p| p.name.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn rel_ids_of_a_frame() {
        let x = r#"<a:graphicData uri="u"><dgm:relIds xmlns:dgm="d" r:dm="rId4" r:lo="rId5" r:qs="rId6" r:cs="rId7"/></a:graphicData>"#;
        assert_eq!(
            super::rel_ids(x),
            Some((
                "rId4".to_owned(),
                [
                    Some("rId5".to_owned()),
                    Some("rId6".to_owned()),
                    Some("rId7".to_owned())
                ]
            ))
        );
        // Image relationships of a diagram part.
        let rels = br#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rIdImg1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image2.png"/><Relationship Id="rId9" Type="http://x/other" Target="x.xml"/></Relationships>"#;
        assert_eq!(
            super::image_rels(rels, "/word/diagrams/data1.xml"),
            vec![("rIdImg1".to_owned(), "/word/media/image2.png".to_owned())]
        );
        assert_eq!(
            super::rels_name("/word/diagrams/data1.xml"),
            "/word/diagrams/_rels/data1.xml.rels"
        );
        assert_eq!(super::rel_ids("<a:graphicData/>"), None);
    }
}
