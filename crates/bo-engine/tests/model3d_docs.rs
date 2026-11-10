//! 3D models in BlueDoc and BlueSheet (CORE-3DMODEL-001): a hand-written document and
//! workbook with an [MS-ODRAWXML] `am3d:model3d` in `mc:AlternateContent` (picture
//! fallback). BlueDoc shows the fallback picture; both keep the model, its GLB part and the
//! markup on save.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::sheet::SheetEngine;
use bo_engine::Engine;
use bo_opc::{Limits, Package, Relationship, TargetMode, REL_OFFICE_DOCUMENT};

const AM3D: &str = "http://schemas.microsoft.com/office/drawing/2017/model3d";
const REL_MODEL3D: &str = "http://schemas.microsoft.com/office/2017/06/relationships/model3d";
const REL_IMAGE: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";

fn png() -> Vec<u8> {
    let scene = bo_model3d::sample::cube();
    let (lo, hi) = scene.bounds().unwrap();
    let mut v = bo_model3d::view::View3d::fit(lo, hi);
    let (w, h) = v.frame_for(&scene, 1_800_000.0, 1_800_000.0);
    bo_model3d::raster::render_png(&scene, &v, w, h, 48, 48)
}

fn rel(id: &str, ty: &str, target: &str) -> Relationship {
    Relationship {
        id: id.into(),
        rel_type: ty.into(),
        target: target.into(),
        mode: TargetMode::Internal,
    }
}

/// The model markup inside a `graphicData` (CT_Model3D, hand-written).
fn model3d(rid_model: &str, rid_image: &str) -> String {
    format!(
        concat!(
            r#"<am3d:model3d r:embed="{m}"><am3d:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1800000" cy="1800000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></am3d:spPr>"#,
            r#"<am3d:camera><am3d:pos x="0" y="0" z="80000000"/><am3d:up dx="0" dy="36000000" dz="0"/><am3d:lookAt x="0" y="0" z="0"/><am3d:perspective fov="1800000"/></am3d:camera>"#,
            r#"<am3d:trans><am3d:meterPerModelUnit n="1" d="1"/><am3d:rot ax="1500000" ay="2100000" az="0"/></am3d:trans>"#,
            r#"<am3d:raster rName="BlueOffice" rVer="1"><am3d:blip r:embed="{i}"/></am3d:raster><am3d:objViewport viewportSz="2000000"/></am3d:model3d>"#
        ),
        m = rid_model,
        i = rid_image
    )
}

#[test]
fn bluedoc_shows_the_fallback_and_keeps_the_model() {
    let drawing = |inner: &str, pic: bool| {
        format!(
            concat!(
                r#"<w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="1800000" cy="1800000"/><wp:docPr id="1" name="3D Model 1" descr="Khối lập phương"/>"#,
                r#"<a:graphic><a:graphicData uri="{uri}">{inner}</a:graphicData></a:graphic></wp:inline></w:drawing>"#
            ),
            uri = if pic {
                "http://schemas.openxmlformats.org/drawingml/2006/picture"
            } else {
                AM3D
            },
            inner = inner
        )
    };
    let pic = r#"<pic:pic><pic:nvPicPr><pic:cNvPr id="0" name="3D Model 1"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="rId3"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="1800000" cy="1800000"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic>"#;
    let body = format!(
        concat!(
            r#"<w:p><w:r><w:t>Mô hình:</w:t></w:r><w:r><mc:AlternateContent><mc:Choice Requires="am3d">{choice}</mc:Choice>"#,
            r#"<mc:Fallback>{fallback}</mc:Fallback></mc:AlternateContent></w:r></w:p>"#
        ),
        choice = drawing(&model3d("rId2", "rId3"), false),
        fallback = drawing(pic, true)
    );
    let xml = format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" "#,
            r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" "#,
            r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture" "#,
            r#"xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006" xmlns:am3d="{am3d}" mc:Ignorable="am3d"><w:body>{body}<w:sectPr/></w:body></w:document>"#
        ),
        am3d = AM3D,
        body = body
    );
    let glb = bo_model3d::glb::write(&bo_model3d::sample::cube());
    let mut p = Package::new();
    p.add_part("/word/document.xml", bo_docx::CT_DOCUMENT, xml.into_bytes())
        .unwrap();
    p.add_part("/word/media/model3d1.glb", "model/gltf-binary", glb.clone())
        .unwrap();
    p.add_part("/word/media/image1.png", "image/png", png())
        .unwrap();
    p.set_relationships(
        "/",
        &[rel("rId1", REL_OFFICE_DOCUMENT, "/word/document.xml")],
    );
    p.set_relationships(
        "/word/document.xml",
        &[
            rel("rId2", REL_MODEL3D, "media/model3d1.glb"),
            rel("rId3", REL_IMAGE, "media/image1.png"),
        ],
    );
    let bytes = p.write().unwrap();
    let mut e = Engine::with_bundled_fonts();
    e.open(&bytes, "model3d.docx").unwrap();
    // The fallback picture is drawn on the page.
    let page = serde_json::to_value(e.page(0).unwrap()).unwrap();
    let images: Vec<String> = page["commands"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|c| c["image"].as_str().map(str::to_owned))
        .collect();
    assert_eq!(images.len(), 1, "{images:?}");
    assert!(e.media(&images[0]).unwrap().starts_with(b"\x89PNG"));
    // Saved: the model markup, its GLB part and relationship are kept.
    let out = e.save_docx().unwrap();
    let pkg = Package::read(&out, &Limits::default()).unwrap();
    let doc = String::from_utf8(pkg.part("/word/document.xml").unwrap().data.clone()).unwrap();
    assert!(
        doc.contains("am3d:model3d") && doc.contains(r#"Requires="am3d""#),
        "{doc}"
    );
    assert_eq!(pkg.part("/word/media/model3d1.glb").unwrap().data, glb);
    assert_eq!(
        pkg.content_types().unwrap().get("/word/media/model3d1.glb"),
        Some("model/gltf-binary")
    );
    let rels = pkg.relationships("/word/document.xml").unwrap();
    assert!(rels.iter().any(|r| r.rel_type == REL_MODEL3D));
}

#[test]
fn bluesheet_keeps_the_model_on_save() {
    let drawing = format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" "#,
            r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">"#,
            r#"<mc:AlternateContent xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><mc:Choice xmlns:am3d="{am3d}" Requires="am3d">"#,
            r#"<xdr:twoCellAnchor editAs="oneCell"><xdr:from><xdr:col>1</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>1</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from>"#,
            r#"<xdr:to><xdr:col>4</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>10</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to>"#,
            r#"<xdr:graphicFrame macro=""><xdr:nvGraphicFramePr><xdr:cNvPr id="2" name="3D Model 1"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr>"#,
            r#"<xdr:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></xdr:xfrm><a:graphic><a:graphicData uri="{am3d}">{model}</a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/></xdr:twoCellAnchor>"#,
            r#"</mc:Choice><mc:Fallback><xdr:twoCellAnchor editAs="oneCell"><xdr:from><xdr:col>1</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>1</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from>"#,
            r#"<xdr:to><xdr:col>4</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>10</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to><xdr:pic><xdr:nvPicPr><xdr:cNvPr id="2" name="3D Model 1"/><xdr:cNvPicPr/></xdr:nvPicPr>"#,
            r#"<xdr:blipFill><a:blip r:embed="rId2"/><a:stretch><a:fillRect/></a:stretch></xdr:blipFill><xdr:spPr><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></xdr:spPr></xdr:pic><xdr:clientData/></xdr:twoCellAnchor>"#,
            r#"</mc:Fallback></mc:AlternateContent></xdr:wsDr>"#
        ),
        am3d = AM3D,
        model = model3d("rId1", "rId2")
    );
    let sheet = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheetData><row r="1"><c r="A1"><v>1</v></c></row></sheetData><drawing r:id="rId1"/></worksheet>"#;
    let wb = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="Trang1" sheetId="1" r:id="rId1"/></sheets></workbook>"#;
    let glb = bo_model3d::glb::write(&bo_model3d::sample::pyramid());
    let ct = "application/vnd.openxmlformats-officedocument.";
    let mut p = Package::new();
    p.add_part(
        "/xl/workbook.xml",
        &format!("{ct}spreadsheetml.sheet.main+xml"),
        wb.as_bytes().to_vec(),
    )
    .unwrap();
    p.add_part(
        "/xl/worksheets/sheet1.xml",
        &format!("{ct}spreadsheetml.worksheet+xml"),
        sheet.as_bytes().to_vec(),
    )
    .unwrap();
    p.add_part(
        "/xl/drawings/drawing1.xml",
        &format!("{ct}drawing+xml"),
        drawing.clone().into_bytes(),
    )
    .unwrap();
    p.add_part("/xl/media/model3d1.glb", "model/gltf-binary", glb.clone())
        .unwrap();
    p.add_part("/xl/media/image1.png", "image/png", png())
        .unwrap();
    p.set_relationships("/", &[rel("rId1", REL_OFFICE_DOCUMENT, "xl/workbook.xml")]);
    p.set_relationships(
        "/xl/workbook.xml",
        &[rel(
            "rId1",
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet",
            "worksheets/sheet1.xml",
        )],
    );
    p.set_relationships(
        "/xl/worksheets/sheet1.xml",
        &[rel(
            "rId1",
            "http://schemas.openxmlformats.org/officeDocument/2006/relationships/drawing",
            "../drawings/drawing1.xml",
        )],
    );
    p.set_relationships(
        "/xl/drawings/drawing1.xml",
        &[
            rel("rId1", REL_MODEL3D, "../media/model3d1.glb"),
            rel("rId2", REL_IMAGE, "../media/image1.png"),
        ],
    );
    let bytes = p.write().unwrap();
    let mut e = SheetEngine::new();
    e.open(&bytes, "model3d.xlsx").unwrap();
    e.set_input(0, 0, 1, "đã sửa").unwrap();
    let out = e.save("xlsx", 0).unwrap();
    let pkg = Package::read(&out, &Limits::default()).unwrap();
    assert_eq!(
        String::from_utf8(pkg.part("/xl/drawings/drawing1.xml").unwrap().data.clone()).unwrap(),
        drawing
    );
    assert_eq!(pkg.part("/xl/media/model3d1.glb").unwrap().data, glb);
    let rels = pkg.relationships("/xl/drawings/drawing1.xml").unwrap();
    assert!(rels.iter().any(|r| r.rel_type == REL_MODEL3D));
}
