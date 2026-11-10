//! BlueSlide 3D models (CORE-3DMODEL-001): insert GLB / OBJ / STL, turn, zoom, pan, preset
//! and reset views, save as [MS-ODRAWXML] `am3d:model3d` + GLB part + fallback picture,
//! reopen with the view kept; the picture is rendered again for each view and resize.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use bo_opc::{Limits, Package};
use std::path::PathBuf;

fn deck(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn open_bytes(bytes: &[u8]) -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.open(bytes, "model.pptx").unwrap();
    e
}

fn run(e: &mut Engine, json: &str) -> serde_json::Value {
    let out = e.slide_command_json(json).unwrap();
    let r: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert!(r.get("error").is_none(), "{json}: {:?}", r.get("error"));
    r
}

fn insert(e: &mut Engine, bytes: &[u8], name: &str) -> serde_json::Value {
    let r = e.slide_insert_picture(bytes.to_vec(), name).unwrap();
    let v = serde_json::to_value(&r).unwrap();
    assert!(v.get("error").is_none(), "{name}: {:?}", v.get("error"));
    v
}

/// Save; with `BO_MODEL3D_OUT=<dir>` the file is also written there (OpenXmlValidator).
fn save(e: &Engine, name: &str) -> Vec<u8> {
    let bytes = e.save_pptx().unwrap();
    if let Ok(dir) = std::env::var("BO_MODEL3D_OUT") {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(PathBuf::from(dir).join(name), &bytes).unwrap();
    }
    bytes
}

fn picture_part(e: &Engine) -> String {
    // The slide's image command shows the model's picture.
    let page = e.page(0).unwrap();
    let json = serde_json::to_value(page).unwrap();
    let cmds = json["commands"].as_array().unwrap();
    cmds.iter()
        .rev()
        .find_map(|c| c["image"].as_str().map(str::to_owned))
        .expect("an image command")
}

#[test]
fn inserted_model_is_turned_saved_and_reopened_with_its_view() {
    let mut e = open_bytes(&deck("slide-edit-text.pptx"));
    run(&mut e, r#"{"cmd":"goTo","slide":0}"#);
    let glb = bo_model3d::glb::write(&bo_model3d::sample::cube());
    let r = insert(&mut e, &glb, "cube.glb");
    let m = &r["model3d"];
    assert_eq!(m["triangles"], 12);
    assert_eq!(m["descr"], "cube");
    let id = m["id"].as_u64().unwrap();
    let part = m["part"].as_str().unwrap().to_owned();
    assert!(
        part.starts_with("/ppt/media/model3d") && part.ends_with(".glb"),
        "{part}"
    );
    assert_eq!(e.media(&part).unwrap(), glb.as_slice());
    assert_eq!(r["selection"][0]["kind"], "model3d");
    let pic0 = picture_part(&e);
    assert!(e.media(&pic0).unwrap().starts_with(b"\x89PNG"));

    // Turn 30° → rot.ay = 1 800 000; a new picture.
    let r = run(&mut e, r#"{"cmd":"model3dRotate","dx":30,"dy":-10}"#);
    assert_eq!(r["model3d"]["view"]["rot"][1], 1_800_000.0);
    assert_eq!(r["model3d"]["view"]["rot"][0], 21_000_000.0);
    let pic1 = picture_part(&e);
    assert_ne!(pic0, pic1);
    // Undo restores the view and the picture.
    let u = run(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(u["model3d"]["view"]["rot"][1], 0.0);
    assert_eq!(picture_part(&e), pic0);
    run(&mut e, r#"{"cmd":"redo"}"#);

    // Zoom and pan change the viewport and the camera.
    let before = run(&mut e, r#"{"cmd":"model3dInfo"}"#)["model3d"]["view"].clone();
    let z = run(&mut e, r#"{"cmd":"model3dZoom","factor":1.25}"#);
    let vp0 = before["viewport"].as_f64().unwrap();
    assert!(
        (z["model3d"]["view"]["viewport"].as_f64().unwrap() - (vp0 * 1.25).round()).abs() < 1.0
    );
    let p = run(&mut e, r#"{"cmd":"model3dPan","dx":10,"dy":0}"#);
    assert_ne!(p["model3d"]["view"]["cameraPos"][0], before["cameraPos"][0]);
    // Presets and reset.
    let t = run(&mut e, r#"{"cmd":"model3dPreset","preset":"top"}"#);
    assert_eq!(
        t["model3d"]["view"]["rot"],
        serde_json::json!([5_400_000.0, 0.0, 0.0])
    );
    let refused: serde_json::Value = serde_json::from_str(
        &e.slide_command_json(r#"{"cmd":"model3dPreset","preset":"nowhere"}"#)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(refused["error"], "badPreset");
    let rs = run(&mut e, r#"{"cmd":"model3dReset"}"#);
    assert_eq!(
        rs["model3d"]["view"]["rot"],
        serde_json::json!([0.0, 0.0, 0.0])
    );
    run(
        &mut e,
        r#"{"cmd":"model3dPreset","preset":"aboveFrontLeft"}"#,
    );
    let view = run(&mut e, r#"{"cmd":"model3dInfo"}"#)["model3d"]["view"].clone();

    // The viewer's mesh.
    let mesh = run(&mut e, r#"{"cmd":"model3dMesh"}"#)["model3dMesh"].clone();
    assert_eq!(mesh["triangles"], 12);
    assert_eq!(mesh["vertices"], 24);

    // A resize renders a picture of the new shape.
    let pic_before = picture_part(&e);
    let rect0 = run(&mut e, r#"{"cmd":"model3dInfo"}"#)["model3d"]["rect"].clone();
    let rr = run(
        &mut e,
        r#"{"cmd":"transform","transform":{"kind":"resize","handle":"e","dx":60,"dy":0},"snap":false}"#,
    );
    let pic_after = picture_part(&e);
    eprintln!("{rect0} -> {}", rr["model3d"]["rect"]);
    assert_ne!(pic_before, pic_after);
    let size = |p: &str| {
        let d = e.media(p).unwrap();
        (
            u32::from_be_bytes([d[16], d[17], d[18], d[19]]),
            u32::from_be_bytes([d[20], d[21], d[22], d[23]]),
        )
    };
    let (w0, h0) = size(&pic_before);
    let (w1, h1) = size(&pic_after);
    assert!(w1 > w0 && h1 == h0, "{w0}×{h0} → {w1}×{h1}");

    // Save → OOXML parts, reopen → same view.
    let bytes = save(&e, "slide-model3d-engine.pptx");
    let pkg = Package::read(&bytes, &Limits::default()).unwrap();
    assert!(pkg.part(&pic0).is_none(), "old pictures are dropped");
    assert!(pkg.part(&pic_after).is_some());
    let ct = pkg.content_types().unwrap();
    assert_eq!(ct.get(&part), Some("model/gltf-binary"));
    let slide_xml =
        String::from_utf8(pkg.part("/ppt/slides/slide1.xml").unwrap().data.clone()).unwrap();
    assert!(slide_xml.contains("am3d:model3d"), "{slide_xml}");
    assert!(slide_xml.contains(r#"Requires="am3d""#));
    assert!(slide_xml.contains("<mc:Fallback><p:pic>"));
    let rels = String::from_utf8(
        pkg.part("/ppt/slides/_rels/slide1.xml.rels")
            .unwrap()
            .data
            .clone(),
    )
    .unwrap();
    assert!(rels.contains("http://schemas.microsoft.com/office/2017/06/relationships/model3d"));

    let mut e2 = open_bytes(&bytes);
    run(&mut e2, r#"{"cmd":"goTo","slide":0}"#);
    let r2 = run(&mut e2, &format!(r#"{{"cmd":"select","ids":[{id}]}}"#));
    assert_eq!(r2["model3d"]["view"], view);
    assert_eq!(r2["model3d"]["triangles"], 12);
    // Unchanged → saved again identically.
    assert_eq!(e2.save_pptx().unwrap(), e2.save_pptx().unwrap());
    let again = e2.save_pptx().unwrap();
    let pkg2 = Package::read(&again, &Limits::default()).unwrap();
    assert_eq!(
        pkg2.part("/ppt/slides/slide1.xml").unwrap().data,
        pkg.part("/ppt/slides/slide1.xml").unwrap().data
    );
}

#[test]
fn obj_stl_and_gltf_are_stored_as_glb() {
    let mut e = open_bytes(&deck("slide-edit-text.pptx"));
    run(&mut e, r#"{"cmd":"goTo","slide":0}"#);
    for (bytes, name, tris) in [
        (
            bo_model3d::sample::pyramid_obj().into_bytes(),
            "pyramid.obj",
            6,
        ),
        (bo_model3d::sample::tetra_stl_binary(), "tetra.stl", 4),
        (
            bo_model3d::sample::tetra_stl_ascii().into_bytes(),
            "tetra-ascii.STL",
            4,
        ),
    ] {
        let r = insert(&mut e, &bytes, name);
        assert_eq!(r["model3d"]["triangles"], tris, "{name}");
        let part = r["model3d"]["part"].as_str().unwrap();
        assert!(e.media(part).unwrap().starts_with(b"glTF"), "{name}");
    }
    // Not a model.
    let bad = e
        .slide_insert_picture(b"solid nothing".to_vec(), "x.stl")
        .unwrap();
    assert_eq!(bad.error.as_deref(), Some("badModel"));
    let bytes = save(&e, "slide-model3d-kinds.pptx");
    let pkg = Package::read(&bytes, &Limits::default()).unwrap();
    let glbs = pkg
        .parts()
        .iter()
        .filter(|p| p.name.ends_with(".glb"))
        .count();
    assert_eq!(glbs, 3);
}

#[test]
fn alt_text_and_name_follow_into_the_fallback() {
    let mut e = open_bytes(&deck("slide-edit-text.pptx"));
    run(&mut e, r#"{"cmd":"goTo","slide":0}"#);
    let glb = bo_model3d::glb::write(&bo_model3d::sample::ring());
    let r = insert(&mut e, &glb, "ring.glb");
    assert!(r["model3d"]["id"].as_u64().is_some());
    let a = run(
        &mut e,
        r#"{"cmd":"model3dAltText","text":"Vòng xuyến xanh"}"#,
    );
    assert_eq!(a["model3d"]["descr"], "Vòng xuyến xanh");
    let bytes = e.save_pptx().unwrap();
    let pkg = Package::read(&bytes, &Limits::default()).unwrap();
    let xml = String::from_utf8(pkg.part("/ppt/slides/slide1.xml").unwrap().data.clone()).unwrap();
    assert_eq!(
        xml.matches(r#"descr="Vòng xuyến xanh""#).count(),
        2,
        "{xml}"
    );
}
