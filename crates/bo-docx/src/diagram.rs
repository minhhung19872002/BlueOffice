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

/// Data model and layout definition of a diagram, with their part names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagramSource {
    /// Data model part (§21.4.3) and its bytes.
    pub data: (String, Vec<u8>),
    /// Layout definition part (§21.4.2.15), if any.
    pub layout: Option<(String, Vec<u8>)>,
}

/// Is `img` a SmartArt frame (its media a diagram drawing or data model part)?
#[must_use]
pub fn is_diagram(doc: &Document, img: &Image) -> bool {
    img.shape.is_none()
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
    if img.src.is_some() {
        return None;
    }
    let names = DiagramPart::ALL.map(|k| diagram::sibling(&img.media, k).unwrap_or_default());
    names
        .iter()
        .all(|n| doc.media.contains_key(n))
        .then_some(names)
}

/// `r:dm` / `r:lo` relationship ids of the `dgm:relIds` in a drawing's XML (§21.4.2.6).
fn rel_ids(xml: &str) -> Option<(String, Option<String>)> {
    let at = xml.find(":relIds")?;
    let tag = &xml[at..at + xml[at..].find('>')?];
    let val = |name: &str| {
        let key = format!(":{name}=\"");
        let s = tag.find(&key)? + key.len();
        Some(tag[s..s + tag[s..].find('"')?].to_owned())
    };
    Some((val("dm")?, val("lo")))
}

/// Data model and layout of SmartArt `img`: from the document's media for diagrams BlueOffice
/// wrote, else through the main document part's relationships in the source package.
#[must_use]
pub fn diagram_source(doc: &Document, img: &Image) -> Option<DiagramSource> {
    let get = |k: DiagramPart| {
        let n = diagram::sibling(&img.media, k)?;
        let b = doc.media.get(&n)?;
        Some((n, b.as_ref().clone()))
    };
    if img.src.is_none() {
        if let Some(data) = get(DiagramPart::Data) {
            return Some(DiagramSource {
                data,
                layout: get(DiagramPart::Layout),
            });
        }
    }
    let src = doc.source.as_ref()?;
    let (dm, lo) = rel_ids(&img.src.as_ref()?.xml)?;
    let pkg = Package::read(&src.bytes, &Limits::default()).ok()?;
    let rels = pkg.relationships(&src.main_part).ok()?;
    let part = |id: &str| {
        let r = rels.iter().find(|r| r.id == id)?;
        let name = bo_opc::resolve_target(&src.main_part, &r.target);
        let p = pkg.part(&name)?;
        Some((name, p.data.clone()))
    };
    Some(DiagramSource {
        data: part(&dm)?,
        layout: lo.as_deref().and_then(part),
    })
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
            Some(("rId4".to_owned(), Some("rId5".to_owned())))
        );
        assert_eq!(super::rel_ids("<a:graphicData/>"), None);
    }
}
