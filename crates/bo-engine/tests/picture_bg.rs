//! Background removal (CORE-PIC-002) in BlueSlide, BlueDoc and BlueSheet: preview,
//! removal as one undo step, the original kept in the file ([MS-ODRAWXML]
//! `a14:imgLayer` + `a14:backgroundRemoval`), reset, and the round trip.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use base64::Engine as _;
use bo_engine::Engine;
use bo_model::slide::ShapeNode;
use std::path::PathBuf;

/// A red disk on a blue → green gradient (PNG, `w`×`h`).
fn disk_png(w: u32, h: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity((w * h * 4) as usize);
    let (cx, cy, r) = (
        f64::from(w) / 2.0,
        f64::from(h) / 2.0,
        f64::from(w.min(h)) * 0.32,
    );
    for y in 0..h {
        for x in 0..w {
            let d = (f64::from(x) + 0.5 - cx).powi(2) + (f64::from(y) + 0.5 - cy).powi(2);
            let t = f64::from(x) / f64::from(w);
            let c = if d <= r * r {
                [210, 45, 40]
            } else {
                [
                    (25.0 + 15.0 * t) as u8,
                    (70.0 + 120.0 * t) as u8,
                    (200.0 - 130.0 * t) as u8,
                ]
            };
            pixels.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    bo_raster::rgba_to_png(bo_raster::codecs::Rgba {
        width: w,
        height: h,
        pixels,
    })
    .unwrap()
}

/// Alpha of the corner and of the centre of a picture.
fn corner_and_centre(png: &[u8]) -> (u8, u8) {
    let img = bo_raster::picfx::decode_rgba(png).unwrap();
    let (w, h) = (img.width as usize, img.height as usize);
    (img.pixels[3], img.pixels[((h / 2) * w + w / 2) * 4 + 3])
}

fn keep_copy(bytes: &[u8], name: &str) {
    // Written files are checked with OpenXmlValidator (`bo-cli validate <file>`); keep a
    // copy when asked.
    if let Ok(dir) = std::env::var("BO_PIC_OUT") {
        std::fs::write(PathBuf::from(dir).join(name), bytes).unwrap();
    }
}

fn deck() -> Engine {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide/slide-shapes-groups.pptx");
    let mut e = Engine::with_bundled_fonts();
    e.open(&std::fs::read(p).unwrap(), "d.pptx").unwrap();
    e
}

fn slide_json(e: &mut Engine, json: &str) -> serde_json::Value {
    let v: serde_json::Value = serde_json::from_str(&e.slide_command_json(json).unwrap()).unwrap();
    assert!(
        v.get("error").is_none_or(serde_json::Value::is_null),
        "{json}: {v}"
    );
    v
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

/// Bytes of the part a slide relationship points at.
fn rel_bytes(e: &Engine, rid: &str) -> Vec<u8> {
    let p = e.presentation().unwrap();
    let s = &p.slides[0];
    let r = s.info.rels.iter().find(|r| r.id == rid).unwrap();
    let name = bo_opc::resolve_target(&s.info.part_name, &r.target);
    p.part_data(&name).unwrap().to_vec()
}

const KEEP_REMOVE: &str = r#""marks":[{"keep":true,"points":[[0.5,0.45],[0.5,0.55]]},{"keep":false,"points":[[0.1,0.9]]}]"#;

#[test]
fn slide_preview_remove_undo_reset_round_trip() {
    let original = disk_png(240, 180);
    let mut e = deck();
    e.slide_insert_picture(original.clone(), "disk.png")
        .unwrap();
    let v = slide_json(&mut e, r#"{"cmd":"pictureInfo"}"#);
    assert_eq!(v["picture"]["removable"], true);
    assert!(v["picture"].get("removal").is_none());
    // Preview: the reduced picture with the removal tinted; the deck is unchanged.
    let v = slide_json(
        &mut e,
        &format!(r#"{{"cmd":"pictureBackgroundPreview",{KEEP_REMOVE}}}"#),
    );
    let bg = &v["picture"]["background"];
    assert_eq!(
        (bg["width"].as_u64(), bg["height"].as_u64()),
        (Some(240), Some(180))
    );
    // The disk is π·57.6² ≈ 10 423 of 43 200 pixels ≈ 24 %.
    let kept = bg["kept"].as_u64().unwrap();
    assert!((22_000..26_500).contains(&kept), "{kept}");
    let preview = base64::engine::general_purpose::STANDARD
        .decode(bg["image"].as_str().unwrap())
        .unwrap();
    let tinted = bo_raster::picfx::decode_rgba(&preview).unwrap();
    assert_ne!(&tinted.pixels[..3], &[25, 70, 200]);
    let before = picture(&e);
    // Remove: one undo step; the picture shows a PNG with alpha.
    let v = slide_json(
        &mut e,
        &format!(r#"{{"cmd":"pictureRemoveBackground",{KEEP_REMOVE}}}"#),
    );
    assert_eq!(
        v["picture"]["removal"]["marks"].as_array().unwrap().len(),
        2
    );
    let pic = picture(&e);
    let shown = rel_bytes(&e, pic.blip.embed.as_deref().unwrap());
    assert_eq!(corner_and_centre(&shown), (0, 255));
    let (layer, settings) =
        bo_drawingml::picture::read_background_removal(&pic.blip.blip_children).unwrap();
    assert_eq!(rel_bytes(&e, layer.as_deref().unwrap()), original);
    assert_eq!(settings.rect, [5000, 5000, 95_000, 95_000]);
    assert_eq!(settings.marks.iter().filter(|m| m.keep).count(), 1);
    // Undo: the original picture, no removal; redo brings the result back.
    slide_json(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(picture(&e), before);
    slide_json(&mut e, r#"{"cmd":"redo"}"#);
    assert_eq!(picture(&e).blip.embed, pic.blip.embed);
    // Saved and reopened: the result, the original and the settings survive.
    let bytes = e.save_pptx().unwrap();
    keep_copy(&bytes, "bg-slide.pptx");
    let mut e2 = Engine::with_bundled_fonts();
    e2.open(&bytes, "x.pptx").unwrap();
    let p2 = picture(&e2);
    assert_eq!(
        corner_and_centre(&rel_bytes(&e2, p2.blip.embed.as_deref().unwrap())),
        (0, 255)
    );
    e2.slide_command_json(&format!(r#"{{"cmd":"select","ids":[{}]}}"#, p2.nv.id))
        .unwrap();
    let v = slide_json(&mut e2, r#"{"cmd":"pictureInfo"}"#);
    assert_eq!(v["picture"]["removal"]["rect"][0], 0.05);
    // Removing again starts from the original (same settings → same result).
    let again = slide_json(
        &mut e2,
        &format!(r#"{{"cmd":"pictureBackgroundPreview",{KEEP_REMOVE}}}"#),
    );
    assert_eq!(again["picture"]["background"]["kept"].as_u64(), Some(kept));
    // Reset picture: the original is shown again, the result part leaves the package.
    slide_json(&mut e2, r#"{"cmd":"pictureReset"}"#);
    let p3 = picture(&e2);
    assert_eq!(rel_bytes(&e2, p3.blip.embed.as_deref().unwrap()), original);
    assert!(bo_drawingml::picture::read_background_removal(&p3.blip.blip_children).is_none());
    let reset = e2.save_pptx().unwrap();
    keep_copy(&reset, "bg-slide-reset.pptx");
    let pkg = bo_opc::Package::read(&reset, &bo_opc::Limits::default()).unwrap();
    let pngs = pkg
        .parts()
        .iter()
        .filter(|p| p.name.starts_with("/ppt/media/") && p.data == original)
        .count();
    assert_eq!(pngs, 1);
    assert!(!pkg
        .parts()
        .iter()
        .any(|p| p.name.starts_with("/ppt/media/")
            && corner_and_centre_opt(&p.data) == Some((0, 255))));
}

fn corner_and_centre_opt(png: &[u8]) -> Option<(u8, u8)> {
    bo_raster::picfx::decode_rgba(png).map(|_| corner_and_centre(png))
}

#[test]
fn slide_remove_twice_keeps_one_original() {
    let original = disk_png(200, 150);
    let mut e = deck();
    e.slide_insert_picture(original.clone(), "disk.png")
        .unwrap();
    slide_json(&mut e, r#"{"cmd":"pictureRemoveBackground"}"#);
    let first = picture(&e).blip.embed.clone();
    slide_json(
        &mut e,
        &format!(r#"{{"cmd":"pictureRemoveBackground",{KEEP_REMOVE}}}"#),
    );
    let pic = picture(&e);
    assert_ne!(pic.blip.embed, first);
    let (layer, _) =
        bo_drawingml::picture::read_background_removal(&pic.blip.blip_children).unwrap();
    assert_eq!(rel_bytes(&e, layer.as_deref().unwrap()), original);
    // The first result is no longer referenced: the slide has the original and one result.
    let p = e.presentation().unwrap();
    let images = p.slides[0]
        .info
        .rels
        .iter()
        .filter(|r| r.rel_type.ends_with("/image"))
        .count();
    let saved = e.save_pptx().unwrap();
    keep_copy(&saved, "bg-slide-twice.pptx");
    let pkg = bo_opc::Package::read(&saved, &bo_opc::Limits::default()).unwrap();
    let media = pkg
        .parts()
        .iter()
        .filter(|p| p.name.starts_with("/ppt/media/"))
        .count();
    assert!(images >= 2);
    // The corpus deck has its own media too; ours are exactly two more.
    let base = bo_opc::Package::read(
        &std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/corpus/slide/slide-shapes-groups.pptx"),
        )
        .unwrap(),
        &bo_opc::Limits::default(),
    )
    .unwrap()
    .parts()
    .iter()
    .filter(|p| p.name.starts_with("/ppt/media/"))
    .count();
    assert_eq!(media, base + 2);
}

fn doc_json(e: &mut Engine, json: &str) -> serde_json::Value {
    let v: serde_json::Value = serde_json::from_str(&e.command_json(json).unwrap()).unwrap();
    assert!(
        v.get("error").is_none_or(serde_json::Value::is_null),
        "{json}: {v}"
    );
    v
}

#[test]
fn doc_remove_reset_round_trip() {
    let original = disk_png(240, 180);
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    e.insert_image(original.clone(), "disk.png").unwrap();
    let v = doc_json(&mut e, r#"{"cmd":"pictureInfo"}"#);
    let first_media = v["picture"]["media"].as_str().unwrap().to_owned();
    let v = doc_json(
        &mut e,
        &format!(r#"{{"cmd":"pictureBackgroundPreview",{KEEP_REMOVE}}}"#),
    );
    assert!(v["picture"]["background"]["kept"].as_u64().unwrap() > 20_000);
    let v = doc_json(
        &mut e,
        &format!(r#"{{"cmd":"pictureRemoveBackground",{KEEP_REMOVE}}}"#),
    );
    let media = v["picture"]["media"].as_str().unwrap().to_owned();
    assert_ne!(media, first_media);
    assert!(v["picture"]["removal"].is_object());
    let shown = e.document().unwrap().media.get(&media).unwrap().clone();
    assert_eq!(corner_and_centre(&shown), (0, 255));
    // Undo / redo: one step.
    let v = doc_json(&mut e, r#"{"cmd":"undo"}"#);
    let _ = v;
    let v = doc_json(&mut e, r#"{"cmd":"pictureInfo"}"#);
    assert_eq!(v["picture"]["media"], first_media.as_str());
    assert!(v["picture"].get("removal").is_none());
    doc_json(&mut e, r#"{"cmd":"redo"}"#);
    // Saved: the blip shows the result, the layer the original.
    let bytes = e.save_docx().unwrap();
    keep_copy(&bytes, "bg-doc.docx");
    let pkg = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
    let xml = String::from_utf8_lossy(&pkg.part("/word/document.xml").unwrap().data).into_owned();
    assert!(xml.contains("<a14:imgLayer r:embed="), "{xml}");
    assert!(
        xml.contains("<a14:backgroundRemoval t=\"5000\" b=\"95000\" l=\"5000\" r=\"95000\">"),
        "{xml}"
    );
    assert!(pkg.parts().iter().any(|p| p.data == original));
    // Reopened: the removal is read back; reset restores the original and the saved
    // file drops the result.
    let mut e2 = Engine::with_bundled_fonts();
    e2.open(&bytes, "x.docx").unwrap();
    doc_json(&mut e2, r#"{"cmd":"selectAll"}"#);
    let v = doc_json(&mut e2, r#"{"cmd":"pictureInfo"}"#);
    assert_eq!(
        v["picture"]["removal"]["marks"].as_array().unwrap().len(),
        2
    );
    let again = e2.save_docx().unwrap();
    keep_copy(&again, "bg-doc-again.docx");
    let v = doc_json(&mut e2, r#"{"cmd":"pictureReset"}"#);
    let back = v["picture"]["media"].as_str().unwrap().to_owned();
    assert_eq!(**e2.document().unwrap().media.get(&back).unwrap(), original);
    let reset = e2.save_docx().unwrap();
    keep_copy(&reset, "bg-doc-reset.docx");
    let pkg = bo_opc::Package::read(&reset, &bo_opc::Limits::default()).unwrap();
    let xml = String::from_utf8_lossy(&pkg.part("/word/document.xml").unwrap().data).into_owned();
    assert!(!xml.contains("backgroundRemoval"), "{xml}");
    let media: Vec<_> = pkg
        .parts()
        .iter()
        .filter(|p| p.name.starts_with("/word/media/"))
        .collect();
    assert_eq!(media.len(), 1);
    assert_eq!(media[0].data, original);
}

#[test]
fn sheet_cell_picture_background() {
    let original = disk_png(120, 90);
    let mut e = bo_engine::SheetEngine::new();
    e.new_workbook("t.xlsx").unwrap();
    let b64 = base64::engine::general_purpose::STANDARD.encode(&original);
    let run = |e: &mut bo_engine::SheetEngine, j: String| -> serde_json::Value {
        serde_json::from_str(&e.exec_json(&j).unwrap()).unwrap_or(serde_json::Value::Null)
    };
    run(
        &mut e,
        format!(
            r#"{{"cmd":"placeImageInCell","sheet":0,"row":1,"col":1,"data":"{b64}","ext":"png"}}"#
        ),
    );
    let p = run(
        &mut e,
        r#"{"cmd":"cellImageBackgroundPreview","sheet":0,"row":1,"col":1}"#.into(),
    );
    assert!(p.to_string().contains("\"kept\""), "{p}");
    run(
        &mut e,
        r#"{"cmd":"cellImageRemoveBackground","sheet":0,"row":1,"col":1}"#.into(),
    );
    let bytes = |e: &bo_engine::SheetEngine| {
        e.workbook().unwrap().sheets[0]
            .cell(1, 1)
            .unwrap()
            .image
            .as_ref()
            .unwrap()
            .data()
            .unwrap()
            .to_vec()
    };
    assert_eq!(corner_and_centre(&bytes(&e)), (0, 255));
    let xlsx = e.save("xlsx", 0).unwrap();
    keep_copy(&xlsx, "bg-sheet.xlsx");
    assert!(e.undo().unwrap());
    assert_eq!(bytes(&e), original);
    // No picture in the cell: refused.
    assert!(e
        .exec_json(r#"{"cmd":"cellImageRemoveBackground","sheet":0,"row":5,"col":5}"#)
        .is_err());
}
