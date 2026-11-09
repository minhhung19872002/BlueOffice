//! Picture tools (CORE-PIC-001) in BlueSlide and BlueDoc: adjustments, crop,
//! compress, change, reset, SVG pictures → shapes; undo and round trip.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::slide_edit::SlideEditResult;
use bo_engine::Engine;
use bo_model::slide::{Geometry, PathCommand, ShapeNode};
use std::path::PathBuf;

/// `w`×`h` RGB PNG with noisy pixels (compresses poorly, like a photo).
fn photo(w: u32, h: u32) -> Vec<u8> {
    let mut png = Vec::new();
    let mut seed = 11u32;
    let data: Vec<u8> = (0..w * h * 3)
        .map(|_| {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            (seed >> 16) as u8
        })
        .collect();
    {
        let mut enc = png::Encoder::new(&mut png, w, h);
        enc.set_color(png::ColorType::Rgb);
        let mut wr = enc.write_header().unwrap();
        wr.write_image_data(&data).unwrap();
    }
    png
}

fn b64(b: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn deck() -> Engine {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide/slide-shapes-groups.pptx");
    let mut e = Engine::with_bundled_fonts();
    e.open(&std::fs::read(p).unwrap(), "d.pptx").unwrap();
    e
}

fn slide(e: &mut Engine, json: &str) -> SlideEditResult {
    let out = e.slide_command_json(json).unwrap();
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(v.get("error").is_none(), "{json}: {v}");
    // Re-run through the typed path for the result struct.
    serde_json::from_value::<serde_json::Value>(v).unwrap();
    e.slide_command(serde_json::from_str(r#"{"cmd":"state"}"#).unwrap())
        .unwrap()
}

fn slide_json(e: &mut Engine, json: &str) -> serde_json::Value {
    serde_json::from_str(&e.slide_command_json(json).unwrap()).unwrap()
}

fn picture(e: &Engine) -> bo_model::slide::Picture {
    let p = e.presentation().unwrap();
    let mut found = None;
    bo_pptx::visit(&p.slides[0].data.shapes, &mut |n| {
        if let ShapeNode::Picture(pic) = n {
            found = Some((**pic).clone());
        }
    });
    found.expect("picture")
}

fn validate(bytes: &[u8], name: &str) {
    // Written files are checked with OpenXmlValidator by hand
    // (`bo-cli validate <file> --openxml require`); keep a copy when asked.
    if let Ok(dir) = std::env::var("BO_PIC_OUT") {
        std::fs::write(PathBuf::from(dir).join(name), bytes).unwrap();
    }
}

#[test]
fn slide_adjust_crop_undo_round_trip() {
    let mut e = deck();
    let r = e.slide_insert_picture(photo(400, 300), "a.png").unwrap();
    assert_eq!(r.selection[0].kind, "picture");
    // Brightness +20 %, contrast −40 %, greyscale.
    let v = slide_json(
        &mut e,
        r#"{"cmd":"pictureFx","fx":{"bright":20000,"contrast":-40000,"sharpen":25000,"saturation":50000,"grayscale":true}}"#,
    );
    assert_eq!(v["picture"]["fx"]["bright"], 20000);
    assert_eq!(v["picture"]["pixelWidth"], 400);
    // The slide draws the picture through a derived id the engine resolves.
    let page = e.slide(0).unwrap();
    let id = page
        .commands
        .iter()
        .find_map(|c| match c {
            bo_render::Command::Image { image, .. } => Some(image.clone()),
            _ => None,
        })
        .unwrap();
    assert!(
        id.ends_with("#fx:b=20000;c=-40000;s=25000;sat=50000;g"),
        "{id}"
    );
    let png = e.media_resolved(&id).unwrap();
    let img = bo_raster::picfx::decode_rgba(&png).unwrap();
    assert!(img.pixels.chunks(4).all(|p| p[0] == p[1] && p[1] == p[2]));
    // Crop to 1:1: 400×300 keeps 75 % of the width (12.5 % each side).
    slide(&mut e, r#"{"cmd":"pictureCropAspect","ratio":"1:1"}"#);
    let pic = picture(&e);
    assert_eq!(pic.blip.src_rect, Some([12_500, 0, 12_500, 0]));
    let (cx, cy) = pic.props.xfrm.as_ref().unwrap().ext.unwrap();
    assert_eq!(cx, cy);
    // Crop to a shape.
    slide(&mut e, r#"{"cmd":"pictureCropShape","preset":"ellipse"}"#);
    assert!(
        matches!(&picture(&e).props.geometry, Some(Geometry::Preset { name, .. }) if name == "ellipse")
    );
    // Save, reopen: adjustments, crop and shape survive.
    let bytes = e.save_pptx().unwrap();
    validate(&bytes, "pic-slide.pptx");
    let mut e2 = Engine::with_bundled_fonts();
    e2.open(&bytes, "x.pptx").unwrap();
    let p2 = picture(&e2);
    let fx = bo_drawingml::picture::read_fx(&p2.blip.blip_children);
    assert_eq!(
        (fx.bright, fx.contrast, fx.grayscale),
        (20_000, -40_000, true)
    );
    assert_eq!((fx.sharpen, fx.saturation), (25_000, Some(50_000)));
    assert_eq!(p2.blip.src_rect, Some([12_500, 0, 12_500, 0]));
    // Undo the shape, the crop and the adjustments.
    for _ in 0..3 {
        slide(&mut e, r#"{"cmd":"undo"}"#);
    }
    let pic = picture(&e);
    assert!(bo_drawingml::picture::read_fx(&pic.blip.blip_children).is_identity());
    assert_eq!(pic.blip.src_rect, None);
}

#[test]
fn slide_compress_change_reset() {
    let mut e = deck();
    e.slide_insert_picture(photo(400, 300), "a.png").unwrap();
    let before = e.save_pptx().unwrap().len();
    // Shown at most 80 % of the slide; 96 ppi makes it smaller in pixels.
    let v = slide_json(
        &mut e,
        r#"{"cmd":"pictureCompress","ppi":96,"jpegQuality":70,"dropCropped":true}"#,
    );
    let rep = &v["picture"]["compressed"];
    assert_eq!(rep["pictures"], 1);
    assert!(rep["after"].as_u64().unwrap() < rep["before"].as_u64().unwrap());
    let media = v["picture"]["media"].as_str().unwrap().to_owned();
    assert!(media.ends_with(".jpeg"), "{media}");
    let after = e.save_pptx().unwrap();
    assert!(after.len() < before, "{} < {before}", after.len());
    validate(&after, "pic-compressed.pptx");
    // The old PNG part is gone from the package.
    let pkg = bo_opc::Package::read(&after, &bo_opc::Limits::default()).unwrap();
    assert!(!pkg
        .parts()
        .iter()
        .any(|p| p.name.starts_with("/ppt/media/") && p.name.ends_with(".png")));
    // Change picture: same frame, the new picture crops to fill it.
    let frame = picture(&e).props.xfrm.clone();
    let v = slide_json(
        &mut e,
        &format!(
            r#"{{"cmd":"pictureChange","data":"{}","name":"b.png"}}"#,
            b64(&photo(100, 100))
        ),
    );
    assert_eq!(v["picture"]["pixelWidth"], 100);
    let pic = picture(&e);
    assert_eq!(pic.props.xfrm, frame);
    // 100×100 into a 4:3 frame: keep 75 % of the height.
    assert_eq!(pic.blip.src_rect, Some([0, 12_500, 0, 12_500]));
    // Reset with size: 100 px at 96 ppi = 952 500 EMU.
    slide(&mut e, r#"{"cmd":"pictureReset","size":true}"#);
    let pic = picture(&e);
    assert_eq!(pic.blip.src_rect, None);
    assert_eq!(pic.props.xfrm.unwrap().ext, Some((952_500, 952_500)));
    validate(&e.save_pptx().unwrap(), "pic-changed.pptx");
}

#[test]
fn slide_svg_to_shapes() {
    let mut e = deck();
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><rect x="0" y="0" width="100" height="100" fill="#ff0000"/><circle cx="150" cy="50" r="50" fill="#0000ff"/><path d="M0 0 L200 100" stroke="#000000" stroke-width="2"/></svg>"##;
    let v = slide_json(
        &mut e,
        &format!(
            r#"{{"cmd":"pictureInsertSvg","data":"{}","name":"logo.svg"}}"#,
            b64(svg)
        ),
    );
    assert_eq!(v["picture"]["svg"], true);
    validate(&e.save_pptx().unwrap(), "pic-svg.pptx");
    let v = slide_json(&mut e, r#"{"cmd":"pictureToShapes"}"#);
    assert!(v.get("error").is_none(), "{v}");
    let p = e.presentation().unwrap();
    let group = p.slides[0]
        .data
        .shapes
        .iter()
        .rev()
        .find_map(|n| match n {
            ShapeNode::Group(g) => Some(g),
            _ => None,
        })
        .unwrap();
    assert_eq!(group.children.len(), 3);
    // The rectangle: path in EMU (9525 per px), from its own corner.
    let ShapeNode::Shape(rect) = &group.children[0] else {
        panic!()
    };
    let Some(Geometry::Custom(g)) = &rect.props.geometry else {
        panic!()
    };
    assert_eq!(g.paths[0].w, Some(952_500));
    assert_eq!(
        g.paths[0].commands[1],
        PathCommand::LineTo("952500".into(), "0".into())
    );
    // The circle: four cubic Béziers.
    let ShapeNode::Shape(circle) = &group.children[1] else {
        panic!()
    };
    let Some(Geometry::Custom(g)) = &circle.props.geometry else {
        panic!()
    };
    let cubics = g.paths[0]
        .commands
        .iter()
        .filter(|c| matches!(c, PathCommand::CubicTo(_)))
        .count();
    assert_eq!(cubics, 4);
    let bytes = e.save_pptx().unwrap();
    validate(&bytes, "pic-svg-shapes.pptx");
    slide(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(picture(&e).nv.descr.as_deref(), Some("logo.svg"));
}

fn doc_json(e: &mut Engine, json: &str) -> serde_json::Value {
    serde_json::from_str(&e.command_json(json).unwrap()).unwrap()
}

fn doc_with_picture() -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    e.insert_image(photo(400, 300), "a.png").unwrap();
    e
}

#[test]
fn doc_adjust_crop_round_trip() {
    let mut e = doc_with_picture();
    // The caret follows the inserted picture: it is the tools' picture.
    let v = doc_json(&mut e, r#"{"cmd":"state"}"#);
    assert_eq!(v["format"]["pictureTools"], true);
    let v = doc_json(
        &mut e,
        r#"{"cmd":"pictureFx","fx":{"duotone":[128,16776960],"alpha":60000,"sharpen":-30000}}"#,
    );
    assert!(v.get("error").is_none(), "{v}");
    assert_eq!(v["picture"]["fx"]["duotone"][1], 16_776_960);
    let v = doc_json(&mut e, r#"{"cmd":"pictureCropAspect","ratio":"16:9"}"#);
    // 400×300 to 16:9 keeps 75 % of the height.
    assert_eq!(
        v["picture"]["crop"],
        serde_json::json!([0, 12500, 0, 12500])
    );
    doc_json(&mut e, r#"{"cmd":"pictureCropShape","preset":"roundRect"}"#);
    // The page draws a derived picture.
    let page = e.page(0).unwrap();
    let id = page
        .commands
        .iter()
        .find_map(|c| match c {
            bo_render::Command::Image { image, .. } => Some(image.clone()),
            _ => None,
        })
        .unwrap();
    assert!(
        id.contains("#fx:") && id.contains("shape=roundRect@0,12500,0,12500"),
        "{id}"
    );
    assert!(e.media_resolved(&id).is_some());
    let bytes = e.save_docx().unwrap();
    validate(&bytes, "pic-doc.docx");
    let mut e2 = Engine::with_bundled_fonts();
    e2.open(&bytes, "x.docx").unwrap();
    let id2 = e2
        .page(0)
        .unwrap()
        .commands
        .iter()
        .find_map(|c| match c {
            bo_render::Command::Image { image, .. } => Some(image.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        id2.split_once('#').unwrap().1,
        id.split_once('#').unwrap().1
    );
    // Saved again unchanged, then a crop edit patches the source XML.
    let again = e2.save_docx().unwrap();
    validate(&again, "pic-doc-again.docx");
    doc_json(&mut e, r#"{"cmd":"undo"}"#);
    doc_json(&mut e, r#"{"cmd":"undo"}"#);
    doc_json(&mut e, r#"{"cmd":"undo"}"#);
    let v = doc_json(&mut e, r#"{"cmd":"pictureInfo"}"#);
    assert!(v["picture"].get("crop").is_none());
    assert_eq!(v["picture"]["shape"], "rect");
}

#[test]
fn doc_compress_and_reopened_edit() {
    let e = doc_with_picture();
    let bytes = e.save_docx().unwrap();
    let mut e = Engine::with_bundled_fonts();
    e.open(&bytes, "x.docx").unwrap();
    // Select everything: the first picture in the selection is the target.
    doc_json(&mut e, r#"{"cmd":"selectAll"}"#);
    let v = doc_json(&mut e, r#"{"cmd":"pictureCompress","ppi":50,"all":true}"#);
    assert!(v.get("error").is_none(), "{v}");
    let rep = &v["picture"]["compressed"];
    assert_eq!(rep["pictures"], 1);
    let out = e.save_docx().unwrap();
    assert!(out.len() < bytes.len());
    validate(&out, "pic-doc-compressed.docx");
    let pkg = bo_opc::Package::read(&out, &bo_opc::Limits::default()).unwrap();
    let media: Vec<&str> = pkg
        .parts()
        .iter()
        .filter(|p| p.name.starts_with("/word/media/"))
        .map(|p| p.name.as_str())
        .collect();
    assert_eq!(media.len(), 1, "{media:?}");
    // A reopened picture keeps its source XML; a colour change patches it.
    let mut e3 = Engine::with_bundled_fonts();
    e3.open(&out, "y.docx").unwrap();
    doc_json(&mut e3, r#"{"cmd":"selectAll"}"#);
    let v = doc_json(&mut e3, r#"{"cmd":"pictureFx","fx":{"bilevel":50000}}"#);
    assert!(v.get("error").is_none(), "{v}");
    let patched = e3.save_docx().unwrap();
    validate(&patched, "pic-doc-patched.docx");
    let pkg = bo_opc::Package::read(&patched, &bo_opc::Limits::default()).unwrap();
    let xml = String::from_utf8_lossy(&pkg.part("/word/document.xml").unwrap().data).into_owned();
    assert!(xml.contains("<a:biLevel thresh=\"50000\"/>"), "{xml}");
}
