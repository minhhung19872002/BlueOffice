//! Embedded and linked objects on slides (CORE-EMBED-001).
//!
//! A graphic frame (§19.3.1.21) whose `graphicData@uri` is [`URI_OLE`] holds a
//! `p:oleObj` (ISO/IEC 29500-1, §19.3.2.4: `progId`, `name`, `showAsIcon`, `imgW`,
//! `imgH`, `r:id` of the embedded part or linked file) with one of `p:embed`
//! (§19.3.2.2) / `p:link` (§19.3.2.3, `updateAutomatic`), then the preview picture
//! `p:pic` (§19.3.1.37). Files often wrap the frame in `mc:AlternateContent`
//! (ECMA-376 Part 3, §10.2.1) whose fallback carries the `p:pic`. The object itself
//! is kept as is and never run.

use bo_model::slide::XmlElement;

/// `graphicData@uri` of an OLE object frame.
pub const URI_OLE: &str = "http://schemas.openxmlformats.org/presentationml/2006/ole";

/// The `p:oleObj` of a frame's `graphicData` (directly, or the fallback of an
/// `mc:AlternateContent` inside it).
pub fn ole_obj(gd: &XmlElement) -> Option<&XmlElement> {
    if let Some(o) = gd.elements().find(|e| e.local() == "oleObj") {
        return Some(o);
    }
    gd.elements()
        .filter(|e| e.local() == "AlternateContent")
        .flat_map(XmlElement::elements)
        .flat_map(XmlElement::elements)
        .find(|e| e.local() == "oleObj" && e.elements().any(|c| c.local() == "pic"))
}

/// The preview picture (`p:pic`) of an OLE object frame.
pub fn ole_picture(gd: &XmlElement) -> Option<&XmlElement> {
    ole_obj(gd)?.elements().find(|e| e.local() == "pic")
}

/// What a `p:oleObj` says about its object.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OleRef {
    /// `progId`.
    pub prog_id: String,
    /// `r:id` of the embedded part or linked file.
    pub rid: String,
    /// `p:link` (else `p:embed`).
    pub link: bool,
    /// `p:link@updateAutomatic`.
    pub auto_update: bool,
    /// `showAsIcon`.
    pub icon: bool,
    /// `r:embed` of the preview picture's blip.
    pub preview_rid: Option<String>,
}

fn attr_local<'a>(el: &'a XmlElement, local: &str) -> Option<&'a str> {
    el.attrs
        .iter()
        .find(|(k, _)| k.rsplit(':').next() == Some(local))
        .map(|(_, v)| v.as_str())
}

fn find<'a>(el: &'a XmlElement, local: &str) -> Option<&'a XmlElement> {
    if el.local() == local {
        return Some(el);
    }
    el.elements().find_map(|c| find(c, local))
}

/// Read the `p:oleObj` of a frame's `graphicData`.
pub fn ole_ref(gd: &XmlElement) -> Option<OleRef> {
    let o = ole_obj(gd)?;
    let link = o.elements().find(|e| e.local() == "link");
    let truthy = |v: Option<&str>| matches!(v, Some("1" | "true"));
    Some(OleRef {
        prog_id: o.attr("progId").unwrap_or_default().to_owned(),
        rid: o
            .attrs
            .iter()
            .find(|(k, _)| k.ends_with(":id"))
            .map(|(_, v)| v.clone())
            .unwrap_or_default(),
        link: link.is_some(),
        auto_update: truthy(link.and_then(|l| l.attr("updateAutomatic"))),
        icon: truthy(o.attr("showAsIcon")),
        preview_rid: ole_picture(gd)
            .and_then(|p| find(p, "blip"))
            .and_then(|b| attr_local(b, "embed"))
            .map(str::to_owned),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_ole_frame_data() {
        let xml = br#"<a:graphicData xmlns:a="a" xmlns:p="p" xmlns:r="r" uri="http://schemas.openxmlformats.org/presentationml/2006/ole"><mc:AlternateContent xmlns:mc="mc"><mc:Choice Requires="v"><p:oleObj spid="_x0000_s1026" name="W" r:id="rId2" imgW="1" imgH="1" progId="Excel.Sheet.12"><p:embed/></p:oleObj></mc:Choice><mc:Fallback><p:oleObj name="W" r:id="rId2" imgW="1" imgH="1" progId="Excel.Sheet.12"><p:embed/><p:pic><p:nvPicPr><p:cNvPr id="0" name=""/><p:cNvPicPr/><p:nvPr/></p:nvPicPr><p:blipFill><a:blip r:embed="rId3"/><a:stretch><a:fillRect/></a:stretch></p:blipFill><p:spPr/></p:pic></p:oleObj></mc:Fallback></mc:AlternateContent></a:graphicData>"#;
        let gd = bo_drawingml::dom::parse(xml, "/ppt/slides/slide1.xml").unwrap();
        let r = ole_ref(&gd).unwrap();
        assert_eq!(r.prog_id, "Excel.Sheet.12");
        assert_eq!(r.rid, "rId2");
        assert_eq!(r.preview_rid.as_deref(), Some("rId3"));
        assert!(!r.link && !r.icon);
        assert!(ole_picture(&gd).is_some());
    }
}
