//! SmartArt diagram parts shared by BlueDoc and BlueSheet (CORE-SMART-005).
//!
//! A diagram is a graphic frame whose `a:graphicData@uri` is the DrawingML diagram namespace and
//! whose `dgm:relIds` (ECMA-376 Part 1, §21.4.2.6) name four parts by relationship: the data model
//! (`r:dm`, §21.4.3), the layout definition (`r:lo`, §21.4.2.15), the quick style (`r:qs`,
//! §21.4.4) and the colours (`r:cs`, §21.4.4.7). A fifth part, the pre-rendered drawing
//! ([MS-ODRAWXML] `dsp:drawing`, relationship type `diagramDrawing`), is named by the data
//! model's `dsp:dataModelExt@relId`.
//!
//! Diagrams BlueOffice writes name their parts `<dir>/<kind><n>.xml` with one `n` for all five
//! (e.g. `/word/diagrams/data3.xml` … `/word/diagrams/drawing3.xml`); [`sibling`] maps one to
//! another. The data model is built with [`DRAWING_REL_PLACEHOLDER`] as the drawing relationship
//! id and the writers replace it ([`set_drawing_rel`]) once the relationship exists.

use serde::{Deserialize, Serialize};

/// `graphicData@uri` of a SmartArt diagram (ECMA-376 Part 1, §21.4).
pub const URI_DIAGRAM: &str = "http://schemas.openxmlformats.org/drawingml/2006/diagram";
/// DrawingML diagram namespace (`dgm:`, §21.4).
pub const NS_DGM: &str = URI_DIAGRAM;
/// Relationship id written in a new data model until the writer knows the real one.
pub const DRAWING_REL_PLACEHOLDER: &str = "rIdBoDiagramDrawing";

const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";
const CT: &str = "application/vnd.openxmlformats-officedocument.drawingml.";

/// The five parts of a diagram.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagramPart {
    /// Data model (`r:dm`, §21.4.3 `dgm:dataModel`).
    Data,
    /// Layout definition (`r:lo`, §21.4.2.15 `dgm:layoutDef`).
    Layout,
    /// Quick style (`r:qs`, §21.4.4 `dgm:styleDef`).
    Style,
    /// Colours (`r:cs`, §21.4.4.7 `dgm:colorsDef`).
    Colors,
    /// Drawing ([MS-ODRAWXML] `dsp:drawing`).
    Drawing,
}

impl DiagramPart {
    /// All parts, in `dgm:relIds` order then the drawing.
    pub const ALL: [DiagramPart; 5] = [
        DiagramPart::Data,
        DiagramPart::Layout,
        DiagramPart::Style,
        DiagramPart::Colors,
        DiagramPart::Drawing,
    ];

    /// File name stem of the part (`data`, `layout`, `quickStyle`, `colors`, `drawing`).
    #[must_use]
    pub fn stem(self) -> &'static str {
        match self {
            DiagramPart::Data => "data",
            DiagramPart::Layout => "layout",
            DiagramPart::Style => "quickStyle",
            DiagramPart::Colors => "colors",
            DiagramPart::Drawing => "drawing",
        }
    }

    /// Relationship type from the part holding the frame (ECMA-376 Part 1, §21.4; [MS-ODRAWXML]
    /// 2.1.x diagramDrawing).
    #[must_use]
    pub fn rel_type(self) -> String {
        match self {
            DiagramPart::Data => format!("{REL}diagramData"),
            DiagramPart::Layout => format!("{REL}diagramLayout"),
            DiagramPart::Style => format!("{REL}diagramQuickStyle"),
            DiagramPart::Colors => format!("{REL}diagramColors"),
            DiagramPart::Drawing => {
                "http://schemas.microsoft.com/office/2007/relationships/diagramDrawing".to_owned()
            }
        }
    }

    /// Content type of the part (ECMA-376 Part 1, §21.4; [MS-ODRAWXML]).
    #[must_use]
    pub fn content_type(self) -> String {
        match self {
            DiagramPart::Data => format!("{CT}diagramData+xml"),
            DiagramPart::Layout => format!("{CT}diagramLayout+xml"),
            DiagramPart::Style => format!("{CT}diagramStyle+xml"),
            DiagramPart::Colors => format!("{CT}diagramColors+xml"),
            DiagramPart::Drawing => "application/vnd.ms-office.drawingml.diagramDrawing+xml".into(),
        }
    }
}

/// Kind, directory and number of a part named `<dir>/<stem><n>.xml` (`n` may be empty).
#[must_use]
pub fn split_name(name: &str) -> Option<(DiagramPart, &str, &str)> {
    let (dir, file) = name.rsplit_once('/')?;
    let base = file.strip_suffix(".xml")?;
    // `quickStyle` before the shorter stems is irrelevant: no stem is a prefix of another.
    DiagramPart::ALL.iter().find_map(|k| {
        let n = base.strip_prefix(k.stem())?;
        n.bytes()
            .all(|b| b.is_ascii_digit())
            .then_some((*k, dir, n))
    })
}

/// Part `kind` of the diagram that part `name` belongs to (same directory and number).
#[must_use]
pub fn sibling(name: &str, kind: DiagramPart) -> Option<String> {
    let (_, dir, n) = split_name(name)?;
    Some(format!("{dir}/{}{n}.xml", kind.stem()))
}

/// The five part names of diagram number `n` in `dir` (e.g. `/word/diagrams`).
#[must_use]
pub fn part_names(dir: &str, n: usize) -> [String; 5] {
    DiagramPart::ALL.map(|k| format!("{dir}/{}{n}.xml", k.stem()))
}

/// Is `bytes` a diagram data model or drawing part (by its root element)?
#[must_use]
pub fn is_diagram_xml(bytes: &[u8]) -> bool {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]);
    head.contains("<dsp:drawing")
        || head.contains(":drawing ") && head.contains("diagram")
        || head.contains(":dataModel ") && head.contains("drawingml/2006/diagram")
}

/// The data model `data` with the drawing relationship id of its `dsp:dataModelExt@relId`
/// ([MS-ODRAWXML]) set to `rid`; unchanged when it has none.
#[must_use]
pub fn set_drawing_rel(data: &[u8], rid: &str) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(data) else {
        return data.to_vec();
    };
    let Some(ext) = text.find("dataModelExt") else {
        return data.to_vec();
    };
    let Some(at) = text[ext..].find("relId=\"").map(|i| ext + i + 7) else {
        return data.to_vec();
    };
    let Some(end) = text[at..].find('"').map(|i| at + i) else {
        return data.to_vec();
    };
    let mut out = String::with_capacity(text.len() + rid.len());
    out.push_str(&text[..at]);
    out.push_str(rid);
    out.push_str(&text[end..]);
    out.into_bytes()
}

/// `dgm:relIds` (§21.4.2.6) naming the four parts by relationship id, namespaces declared.
#[must_use]
pub fn rel_ids_xml(dm: &str, lo: &str, qs: &str, cs: &str) -> String {
    format!(
        "<dgm:relIds xmlns:dgm=\"{NS_DGM}\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" r:dm=\"{dm}\" r:lo=\"{lo}\" r:qs=\"{qs}\" r:cs=\"{cs}\"/>"
    )
}

/// The parts of a diagram BlueOffice created or edited, by kind (bytes as written, the data model
/// naming [`DRAWING_REL_PLACEHOLDER`] or the drawing relationship it already has).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramXml {
    /// Data model part.
    pub data: Vec<u8>,
    /// Layout definition part.
    pub layout: Vec<u8>,
    /// Quick style part.
    pub style: Vec<u8>,
    /// Colours part.
    pub colors: Vec<u8>,
    /// Drawing part.
    pub drawing: Vec<u8>,
}

impl DiagramXml {
    /// Bytes of part `kind`.
    #[must_use]
    pub fn get(&self, kind: DiagramPart) -> &[u8] {
        match kind {
            DiagramPart::Data => &self.data,
            DiagramPart::Layout => &self.layout,
            DiagramPart::Style => &self.style,
            DiagramPart::Colors => &self.colors,
            DiagramPart::Drawing => &self.drawing,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_siblings() {
        let n = part_names("/word/diagrams", 3);
        assert_eq!(n[0], "/word/diagrams/data3.xml");
        assert_eq!(n[2], "/word/diagrams/quickStyle3.xml");
        assert_eq!(n[4], "/word/diagrams/drawing3.xml");
        assert_eq!(
            sibling("/xl/diagrams/drawing12.xml", DiagramPart::Layout).as_deref(),
            Some("/xl/diagrams/layout12.xml")
        );
        assert_eq!(
            split_name("/word/diagrams/colors.xml").map(|s| s.0),
            Some(DiagramPart::Colors)
        );
        assert!(split_name("/word/media/image1.png").is_none());
        assert!(split_name("/word/diagrams/dataX.xml").is_none());
    }

    #[test]
    fn drawing_rel_is_replaced() {
        let d = format!(
            r#"<dgm:dataModel><dgm:extLst><a:ext uri="u"><dsp:dataModelExt relId="{DRAWING_REL_PLACEHOLDER}" minVer="m"/></a:ext></dgm:extLst></dgm:dataModel>"#
        );
        let out = String::from_utf8(set_drawing_rel(d.as_bytes(), "rId9")).unwrap_or_default();
        assert!(out.contains(r#"<dsp:dataModelExt relId="rId9" minVer="m"/>"#));
        assert_eq!(
            set_drawing_rel(b"<dgm:dataModel/>", "rId1"),
            b"<dgm:dataModel/>"
        );
    }

    #[test]
    fn rel_types_and_content_types() {
        assert!(DiagramPart::Data.rel_type().ends_with("/diagramData"));
        assert_eq!(
            DiagramPart::Style.content_type(),
            "application/vnd.openxmlformats-officedocument.drawingml.diagramStyle+xml"
        );
        assert!(is_diagram_xml(
            br#"<dgm:dataModel xmlns:dgm="http://schemas.openxmlformats.org/drawingml/2006/diagram">"#
        ));
        assert!(!is_diagram_xml(b"\x89PNG"));
    }
}
