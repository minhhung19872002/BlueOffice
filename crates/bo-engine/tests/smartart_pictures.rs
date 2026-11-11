//! Diagram nodes with pictures, editing a node on the canvas, the layout previews and the file's
//! own quick style / colours in BlueSlide (CORE-SMART-003 / 004).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use base64::Engine as _;
use bo_engine::slide_edit::{SlideCommand, SlideEditResult};
use bo_engine::Engine;
use bo_model::slide::{Fill, ShapeNode};
use std::path::PathBuf;

fn deck(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn open(name: &str) -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.open(&deck(name), name).unwrap();
    e
}

fn try_run(e: &mut Engine, json: &str) -> SlideEditResult {
    let cmd: SlideCommand =
        serde_json::from_str(json).unwrap_or_else(|err| panic!("{json}: {err}"));
    e.slide_command(cmd).unwrap()
}

fn run(e: &mut Engine, json: &str) -> SlideEditResult {
    let r = try_run(e, json);
    assert!(r.error.is_none(), "{json}: {:?}", r.error);
    r
}

/// A `w` × `h` PNG of one colour as a `data:` URL.
fn png_url(w: u32, h: u32, rgb: [u8; 3]) -> String {
    let rgba: Vec<u8> = (0..w * h)
        .flat_map(|_| [rgb[0], rgb[1], rgb[2], 255])
        .collect();
    let png = bo_model3d::scene::encode_png(w, h, &rgba);
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(png)
    )
}

fn part<'a>(e: &'a Engine, prefix: &str) -> Vec<&'a bo_model::slide::RawPart> {
    e.presentation()
        .unwrap()
        .parts
        .iter()
        .filter(|p| p.name.starts_with(prefix))
        .collect()
}

// source: ECMA-376 Part 1, §20.1.8.14 blipFill in `dgm:pt/dgm:spPr` and in the drawing part's shape
// ([MS-ODRAWXML] dsp:sp), each part with its own image relationship (Part 2, relationships).
#[test]
fn picture_nodes_are_stored_drawn_replaced_removed_and_kept() {
    let mut e = open("slide-text-bullets.pptx");
    let (red, blue) = (png_url(40, 10, [200, 30, 30]), png_url(8, 8, [30, 30, 200]));
    let r = run(
        &mut e,
        &format!(
            r#"{{"cmd":"insertSmartArt","layout":"pictureStrip","items":[{{"text":"Hà Nội","picture":"{red}"}},{{"text":"Huế"}},{{"text":"Đà Nẵng","picture":"{blue}"}}]}}"#
        ),
    );
    let sa = r.smartart.clone().unwrap();
    let slide = r.slide;
    assert_eq!(
        sa.items
            .iter()
            .map(|i| i.picture.as_deref())
            .collect::<Vec<_>>(),
        [Some("rIdImg1"), None, Some("rIdImg2")]
    );
    // Two image parts; the data model part and the drawing part each name them by relationship.
    let before = part(&e, "/ppt/media/").len();
    let rels = part(&e, "/ppt/diagrams/_rels/");
    assert_eq!(rels.len(), 2);
    for r in &rels {
        let x = String::from_utf8_lossy(&r.data);
        assert!(
            x.contains(r#"Id="rIdImg1""#) && x.contains(r#"Id="rIdImg2""#),
            "{x}"
        );
        assert!(
            x.contains("relationships/image\" Target=\"../media/image"),
            "{x}"
        );
    }
    let data = String::from_utf8_lossy(&part(&e, "/ppt/diagrams/data")[0].data).into_owned();
    assert_eq!(data.matches("<a:blip r:embed=\"rIdImg").count(), 2);
    let drawing = String::from_utf8_lossy(&part(&e, "/ppt/diagrams/drawing")[0].data).into_owned();
    // The 40 × 10 picture is cropped to its round (square) frame: 3/8 from each side.
    assert!(
        drawing.contains(r#"<a:blip r:embed="rIdImg1"/><a:srcRect l="37500" r="37500"/>"#),
        "{drawing}"
    );
    assert!(drawing.contains(r#"<a:blip r:embed="rIdImg2"/><a:stretch>"#));
    // Drawn: the slide's display list shows both images.
    let page = e.slide(slide).unwrap().to_json();
    assert_eq!(page.matches("/ppt/media/image").count(), 2, "{page}");
    // Replace the first picture, remove the third, give the second one the old first picture.
    let green = png_url(4, 4, [30, 200, 30]);
    let r = run(
        &mut e,
        &format!(
            r#"{{"cmd":"setSmartArt","items":[{{"text":"Hà Nội","picture":"{green}"}},{{"text":"Huế","picture":"rIdImg1"}},{{"text":"Đà Nẵng"}}]}}"#
        ),
    );
    let items = r.smartart.unwrap().items;
    assert_eq!(
        items
            .iter()
            .map(|i| i.picture.as_deref())
            .collect::<Vec<_>>(),
        [Some("rIdImg3"), Some("rIdImg1"), None]
    );
    // The removed picture's part is gone, the new one is there.
    assert_eq!(part(&e, "/ppt/media/").len(), before);
    // Changing the layout or the colours keeps the pictures.
    let r = run(
        &mut e,
        r#"{"cmd":"setSmartArt","layout":"picture","colors":"green"}"#,
    );
    assert_eq!(r.smartart.unwrap().items, items);
    // Saved and reopened: the same pictures on the same nodes, still drawn.
    let bytes = e.save_pptx().unwrap();
    if let Ok(dir) = std::env::var("BO_SMARTART_OUT") {
        std::fs::write(format!("{dir}/smartart-slide-pictures.pptx"), &bytes).unwrap();
    }
    let mut back = Engine::with_bundled_fonts();
    back.open(&bytes, "back.pptx").unwrap();
    let r2 = run(
        &mut back,
        &format!(r#"{{"cmd":"select","slide":{slide},"ids":[{}]}}"#, sa.id),
    );
    assert_eq!(r2.smartart.unwrap().items, items);
    assert_eq!(
        back.slide(slide)
            .unwrap()
            .to_json()
            .matches("/ppt/media/image")
            .count(),
        2
    );
    let pkg = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
    for p in pkg
        .parts()
        .iter()
        .filter(|p| p.name.contains("/diagrams/") && !p.name.contains("_rels"))
    {
        if p.name.contains("/data") || p.name.contains("/drawing") {
            let rels = pkg.relationships(&p.name).unwrap();
            assert_eq!(rels.len(), 2, "{}", p.name);
            for r in rels {
                let target = bo_opc::resolve_target(&p.name, &r.target);
                assert!(pkg.part(&target).is_some(), "{target}");
            }
        }
    }
    // Convert to shapes: the pictures become picture fills with relationships of the slide.
    let r = run(&mut e, r#"{"cmd":"smartArtToShapes"}"#);
    assert_eq!(r.selection[0].kind, "group");
    let p = e.presentation().unwrap();
    let s = &p.slides[slide];
    let group = s
        .data
        .shapes
        .iter()
        .find_map(|n| match n {
            ShapeNode::Group(g) if g.nv.id == sa.id => Some(g),
            _ => None,
        })
        .unwrap();
    let embeds: Vec<String> = group
        .children
        .iter()
        .filter_map(|n| match n {
            ShapeNode::Shape(s) => match &s.props.fill {
                Some(Fill::Picture(b)) => b.embed.clone(),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(embeds.len(), 2);
    for rid in &embeds {
        let rel = s.info.rels.iter().find(|r| &r.id == rid).unwrap();
        assert!(rel.rel_type.ends_with("/image"), "{rel:?}");
        let target = bo_opc::resolve_target(&s.info.part_name, &rel.target);
        assert!(p.part_data(&target).is_some(), "{target}");
    }
    assert_eq!(
        e.slide(slide)
            .unwrap()
            .to_json()
            .matches("/ppt/media/image")
            .count(),
        2
    );
    // One undo: the diagram is back with its pictures.
    let r = run(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(r.smartart.unwrap().items, items);
    // Refusals: an id the diagram does not have, a picture that is no PNG / JPEG.
    for bad in ["rId99", "data:image/png;base64,QUJD"] {
        let r = try_run(
            &mut e,
            &format!(r#"{{"cmd":"setSmartArt","items":[{{"text":"A","picture":"{bad}"}}]}}"#),
        );
        assert_eq!(r.error.as_deref(), Some("badPicture"));
    }
}

#[test]
fn a_node_is_found_and_its_text_edited_in_one_undo_step() {
    let mut e = open("slide-text-bullets.pptx");
    let r = run(
        &mut e,
        r#"{"cmd":"insertSmartArt","layout":"process","rect":{"x":100,"y":100,"w":600,"h":200},"items":[{"text":"Tiếp nhận"},{"text":"Xử lý"},{"text":"Trả kết quả"}]}"#,
    );
    let slide = r.slide;
    // Node 1 by index: the box of its shape, inside the frame.
    let r = run(&mut e, r#"{"cmd":"smartArtNodeAt","node":1}"#);
    let n = r.smartart_node.unwrap();
    assert_eq!((n.node, n.text.as_str()), (1, "Xử lý"));
    assert!(n.x >= 100.0 && n.x + n.w <= 700.0 && n.y >= 100.0 && n.y + n.h <= 300.0);
    // The middle of the frame is the middle step; its box is the same.
    let r = run(&mut e, r#"{"cmd":"smartArtNodeAt","x":400,"y":200}"#);
    assert_eq!(r.smartart_node.unwrap(), n);
    // A point of the frame beside every step (the top edge): no node.
    let r = run(&mut e, r#"{"cmd":"smartArtNodeAt","x":400,"y":102}"#);
    assert!(r.smartart_node.is_none());
    let before = run(&mut e, r#"{"cmd":"historyLen"}"#).history.unwrap();
    let r = run(
        &mut e,
        r#"{"cmd":"setSmartArtNodeText","node":1,"text":"Giám sát"}"#,
    );
    let texts: Vec<String> = r
        .smartart
        .unwrap()
        .items
        .into_iter()
        .map(|i| i.text)
        .collect();
    assert_eq!(texts, ["Tiếp nhận", "Giám sát", "Trả kết quả"]);
    assert!(e.slide(slide).unwrap().to_json().contains("Giám"));
    assert_eq!(
        run(&mut e, r#"{"cmd":"historyLen"}"#).history.unwrap(),
        before + 1
    );
    let r = run(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(r.smartart.unwrap().items[1].text, "Xử lý");
    // An empty text or a node that is not there is refused.
    for bad in [
        r#"{"cmd":"setSmartArtNodeText","node":1,"text":"  "}"#,
        r#"{"cmd":"setSmartArtNodeText","node":7,"text":"x"}"#,
    ] {
        assert_eq!(try_run(&mut e, bad).error.as_deref(), Some("badSmartArt"));
    }
    // Layout previews for the dialog.
    let r = run(&mut e, r#"{"cmd":"smartArtPreviews","colors":"green"}"#);
    let list = r.smartart_previews.unwrap();
    assert_eq!(list.as_array().unwrap().len(), 17);
    assert!(list[0]["svg"].as_str().unwrap().contains("#2E8B57"));
    assert!(
        try_run(&mut e, r#"{"cmd":"smartArtPreviews","colors":"rainbow"}"#)
            .error
            .is_some()
    );
}
