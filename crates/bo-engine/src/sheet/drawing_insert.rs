//! Shapes and pictures inserted from the Insert tab (BlueSheet native, round 16): one `xdr:sp`
//! in a group (`xdr:grpSp`), the form [`bo_model::sheet::SheetShapeGroup`] keeps and the xlsx
//! writer places in the sheet's drawing part with relationships for its pictures.

/// File extension of picture `bytes` by their signature (PNG, JPEG, GIF, BMP).
pub(super) fn image_ext(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        Some("jpeg")
    } else if bytes.starts_with(b"GIF8") {
        Some("gif")
    } else if bytes.starts_with(b"BM") {
        Some("bmp")
    } else {
        None
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// The `xdr:grpSp` of one shape `n`: preset geometry `geom` (ECMA-376 Part 1, §20.1.9.18
/// prstGeom), filled with picture part `picture` (§20.1.8.14 blipFill, the writer turns the
/// part name into a relationship) or with the theme's accent colour and its dark outline
/// (§20.1.8.54 solidFill, §20.1.2.2.24 ln), and `text` centred (§20.5.2.35 txBody). The
/// group's child extent equals its extent, so the shape fills the anchor.
pub(super) fn group_xml(
    n: u32,
    geom: &str,
    picture: Option<&str>,
    text: Option<&str>,
    is_picture: bool,
) -> String {
    let geom: String = geom.chars().filter(char::is_ascii_alphanumeric).collect();
    let geom = if geom.is_empty() {
        "rect".to_owned()
    } else {
        geom
    };
    let (cx, cy) = (1_000_000, 1_000_000);
    let fill = match picture {
        Some(p) => format!(
            r#"<a:blipFill><a:blip r:embed="{}"/><a:stretch><a:fillRect/></a:stretch></a:blipFill>"#,
            esc(p)
        ),
        None => r#"<a:solidFill><a:schemeClr val="accent1"/></a:solidFill><a:ln w="12700"><a:solidFill><a:schemeClr val="accent1"><a:shade val="50000"/></a:schemeClr></a:solidFill></a:ln>"#.to_owned(),
    };
    let name = if is_picture { "Picture" } else { "Shape" };
    let body = match text.filter(|t| !t.is_empty()) {
        Some(t) => format!(
            r#"<xdr:txBody><a:bodyPr anchor="ctr"/><a:lstStyle/><a:p><a:pPr algn="ctr"/><a:r><a:rPr lang="en-US" sz="1100"><a:solidFill><a:schemeClr val="lt1"/></a:solidFill></a:rPr><a:t>{}</a:t></a:r></a:p></xdr:txBody>"#,
            esc(t)
        ),
        None => String::new(),
    };
    format!(
        r#"<xdr:grpSp><xdr:nvGrpSpPr><xdr:cNvPr id="{g}" name="Group {n}"/><xdr:cNvGrpSpPr/></xdr:nvGrpSpPr><xdr:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/><a:chOff x="0" y="0"/><a:chExt cx="{cx}" cy="{cy}"/></a:xfrm></xdr:grpSpPr><xdr:sp macro="" textlink=""><xdr:nvSpPr><xdr:cNvPr id="{s}" name="{name} {n}"/><xdr:cNvSpPr/></xdr:nvSpPr><xdr:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="{geom}"><a:avLst/></a:prstGeom>{fill}</xdr:spPr>{body}</xdr:sp></xdr:grpSp>"#,
        g = 2 * n,
        s = 2 * n + 1,
    )
}

#[cfg(test)]
mod tests {
    use super::super::SheetEngine;
    use serde_json::json;

    const PNG_1X1: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

    #[test]
    fn shapes_and_pictures_are_drawn_and_survive_saving() {
        let mut e = SheetEngine::with_bundled_fonts();
        e.new_workbook("Book1.xlsx").unwrap();
        let run = |e: &mut SheetEngine, v: serde_json::Value| e.exec_json(&v.to_string()).unwrap();
        run(
            &mut e,
            json!({ "cmd": "insertDrawing", "sheet": 0, "from": [1, 1], "to": [6, 4],
                "shape": "ellipse", "text": "Hi & bye" }),
        );
        run(
            &mut e,
            json!({ "cmd": "insertDrawing", "sheet": 0, "from": [8, 1], "to": [12, 3],
                "image": PNG_1X1 }),
        );
        let ws = &e.workbook().unwrap().sheets[0];
        assert_eq!(ws.shape_groups.len(), 2);
        assert!(ws.shape_groups[0].grp_sp.contains(r#"prst="ellipse""#));
        assert!(ws.shape_groups[0].grp_sp.contains("Hi &amp; bye"));
        assert_eq!(ws.shape_groups[1].images.len(), 1);
        assert!(e.undo().is_ok());
        assert_eq!(e.workbook().unwrap().sheets[0].shape_groups.len(), 1);
        assert!(e.redo().is_ok());
        let bytes = e.save("xlsx", 0).unwrap();
        let mut back = SheetEngine::with_bundled_fonts();
        back.open(&bytes, "x.xlsx").unwrap();
        let ws = &back.workbook().unwrap().sheets[0];
        assert_eq!(ws.shape_groups.len(), 2);
        assert!(ws.shape_groups.iter().any(|g| g.grp_sp.contains("ellipse")));
        assert!(ws
            .shape_groups
            .iter()
            .any(|g| g.grp_sp.contains("blip") && g.from.row == 8));
        // A bad picture is refused.
        let bad = json!({ "cmd": "insertDrawing", "sheet": 0, "from": [0, 0], "to": [1, 1],
            "image": "AAAA" });
        assert!(e.exec_json(&bad.to_string()).is_err());
    }
}
