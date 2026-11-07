//! BlueSlide editing through the engine facade (Phase 4): every command,
//! unlimited undo/redo (property test), smart guides and lossless saving.

use bo_engine::slide_edit::geom::{place_slide, BoxR};
use bo_engine::slide_edit::{SlideCommand, SlideEditResult};
use bo_engine::Engine;
use bo_model::slide::{Presentation, ShapeNode};
use proptest::prelude::*;
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

fn run(e: &mut Engine, json: &str) -> SlideEditResult {
    let cmd: SlideCommand =
        serde_json::from_str(json).unwrap_or_else(|err| panic!("{json}: {err}"));
    let r = e.slide_command(cmd).unwrap();
    assert!(r.error.is_none(), "{json}: {:?}", r.error);
    r
}

fn pres(e: &Engine) -> Presentation {
    e.presentation().unwrap().clone()
}

/// Centre of a top-level shape that is not a placeholder.
fn free_shapes(e: &Engine, slide: usize) -> Vec<(u32, BoxR)> {
    let p = e.presentation().unwrap();
    place_slide(p, slide)
        .into_iter()
        .filter(|pl| match &p.slides[slide].data.shapes[pl.path[0]] {
            ShapeNode::Shape(s) => s.nv.placeholder.is_none(),
            ShapeNode::Other(_) => false,
            _ => true,
        })
        .map(|pl| (pl.id, pl.b))
        .collect()
}

#[test]
fn click_move_and_save_is_lossless_elsewhere() {
    let mut e = open("slide-shapes-presets.pptx");
    let before = pres(&e);
    let shapes = free_shapes(&e, 0);
    let (id, b) = shapes[3];
    let r = run(
        &mut e,
        &format!(r#"{{"cmd":"click","x":{},"y":{}}}"#, b.cx, b.cy),
    );
    assert_eq!(r.selection.len(), 1);
    assert_eq!(r.selection[0].id, id);
    assert_eq!(r.handles.len(), 9);
    let r = run(
        &mut e,
        r#"{"cmd":"transform","transform":{"kind":"move","dx":13,"dy":-7},"snap":false}"#,
    );
    assert!(r.modified && r.can_undo);
    assert_eq!(r.dirty, vec![0]);
    let nb = r.selection[0].rect;
    assert!((nb.cx - b.cx - 13.0).abs() < 1e-6 && (nb.cy - b.cy + 7.0).abs() < 1e-6);

    // Save and read back: only the moved shape's transform differs.
    let bytes = e.save_pptx().unwrap();
    let back = bo_pptx::read_pptx(&bytes).unwrap();
    assert_eq!(back.slides.len(), before.slides.len());
    for (i, (a, b)) in before.slides.iter().zip(&back.slides).enumerate() {
        if i == 0 {
            continue;
        }
        assert_eq!(a.data, b.data, "slide {i} changed");
    }
    let changed: Vec<usize> = before.slides[0]
        .data
        .shapes
        .iter()
        .zip(&back.slides[0].data.shapes)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(changed.len(), 1);
    let (ShapeNode::Shape(a), ShapeNode::Shape(mut b)) = (
        before.slides[0].data.shapes[changed[0]].clone(),
        back.slides[0].data.shapes[changed[0]].clone(),
    ) else {
        panic!("not a shape");
    };
    let (ox, oy) = a.props.xfrm.as_ref().unwrap().off.unwrap();
    let (nx, ny) = b.props.xfrm.as_ref().unwrap().off.unwrap();
    assert_eq!((nx - ox, ny - oy), (13 * 12_700, -7 * 12_700));
    b.props.xfrm.clone_from(&a.props.xfrm);
    assert_eq!(*a, *b, "only the transform changed");
    // Unmodelled parts survive byte for byte.
    for p in &before.parts {
        if p.name.ends_with("[Content_Types].xml") {
            continue;
        }
        let q = back.parts.iter().find(|q| q.name == p.name).unwrap();
        assert_eq!(p.data, q.data, "{}", p.name);
    }
    // Undo restores the original model; the save flag follows.
    e.mark_saved();
    let r = run(&mut e, r#"{"cmd":"state"}"#);
    assert!(!r.modified);
    let r = run(&mut e, r#"{"cmd":"undo"}"#);
    assert!(r.modified && r.can_redo);
    assert_eq!(pres(&e), before);
}

#[test]
fn smart_guides_snap_to_slide_centre() {
    let mut e = open("slide-shapes-presets.pptx");
    let (_, b) = free_shapes(&e, 0)[0];
    run(
        &mut e,
        &format!(r#"{{"cmd":"click","x":{},"y":{}}}"#, b.cx, b.cy),
    );
    // Ask to move the centre 2 pt short of the slide centre.
    let r = run(&mut e, r#"{"cmd":"state"}"#);
    let w = r.info.pages[0].width as f64;
    let dx = w / 2.0 - b.cx - 2.0;
    let r = run(
        &mut e,
        &format!(
            r#"{{"cmd":"transform","transform":{{"kind":"move","dx":{dx},"dy":0}},"preview":true,"tolerance":4}}"#
        ),
    );
    let pv = r.preview.unwrap();
    assert!((pv.dx - (dx + 2.0)).abs() < 1e-6, "{pv:?}");
    assert!(pv.guides.iter().any(|g| (g.pos - w / 2.0).abs() < 1e-6));
    assert!(!r.modified, "a preview does not change the deck");
    // Resize with the aspect lock through the corner handle.
    let r = run(
        &mut e,
        r#"{"cmd":"transform","transform":{"kind":"resize","handle":"se","dx":30,"dy":0,"keepAspect":true},"snap":false}"#,
    );
    let nb = r.selection[0].rect;
    assert!((nb.w / nb.h - b.w / b.h).abs() < 1e-3);
    assert!((nb.w - b.w - 30.0).abs() < 1e-3);
    // Rotation snapped to 15°.
    let r = run(
        &mut e,
        &format!(
            r#"{{"cmd":"transform","transform":{{"kind":"rotate","x":{},"y":{},"step":true}},"snap":false}}"#,
            nb.cx + 100.0,
            nb.cy + 3.0
        ),
    );
    assert!((r.selection[0].rect.rot - 90.0).abs() < 1e-6);
}

#[test]
fn arrange_group_align_distribute() {
    let mut e = open("slide-shapes-presets.pptx");
    let before = pres(&e);
    let shapes = free_shapes(&e, 0);
    let ids: Vec<u32> = shapes.iter().take(3).map(|s| s.0).collect();
    let boxes: Vec<BoxR> = shapes.iter().take(3).map(|s| s.1).collect();
    run(&mut e, &format!(r#"{{"cmd":"select","ids":{ids:?}}}"#));
    let r = run(&mut e, r#"{"cmd":"group"}"#);
    assert_eq!(r.selection.len(), 1);
    assert_eq!(r.selection[0].kind, "group");
    let gid = r.selection[0].id;
    // Children keep their place inside the group.
    let p = e.presentation().unwrap();
    let placed = place_slide(p, 0);
    let g = placed.iter().find(|x| x.id == gid).unwrap();
    for (c, b) in g.children.iter().zip(&boxes) {
        assert!((c.b.cx - b.cx).abs() < 0.01 && (c.b.cy - b.cy).abs() < 0.01);
    }
    // Clicking a child selects the group; deep click selects the child.
    let r = run(
        &mut e,
        &format!(
            r#"{{"cmd":"click","x":{},"y":{}}}"#,
            boxes[1].cx, boxes[1].cy
        ),
    );
    assert_eq!(r.selection[0].id, gid);
    let r = run(
        &mut e,
        &format!(
            r#"{{"cmd":"hitTest","x":{},"y":{},"deep":true}}"#,
            boxes[1].cx, boxes[1].cy
        ),
    );
    assert_eq!(r.hits, Some(vec![ids[1]]));
    // Move the group, then ungroup: children moved with it.
    run(
        &mut e,
        r#"{"cmd":"transform","transform":{"kind":"move","dx":10,"dy":5},"snap":false}"#,
    );
    let r = run(&mut e, r#"{"cmd":"ungroup"}"#);
    assert_eq!(r.selection.len(), 3);
    for (s, b) in r.selection.iter().zip(&boxes) {
        assert!((s.rect.cx - b.cx - 10.0).abs() < 0.01, "{s:?}");
        assert!((s.rect.cy - b.cy - 5.0).abs() < 0.01);
    }
    let r = run(&mut e, r#"{"cmd":"align","to":"top"}"#);
    let top = r.selection[0].rect.bounds().y;
    assert!(r
        .selection
        .iter()
        .all(|s| (s.rect.bounds().y - top).abs() < 0.01));
    let r = run(&mut e, r#"{"cmd":"distribute","axis":"horizontal"}"#);
    let mut xs: Vec<(f64, f64)> = r
        .selection
        .iter()
        .map(|s| (s.rect.bounds().x, s.rect.bounds().right()))
        .collect();
    xs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let g1 = xs[1].0 - xs[0].1;
    let g2 = xs[2].0 - xs[1].1;
    assert!((g1 - g2).abs() < 0.01, "{xs:?}");
    // Z-order: bring to front puts the selection last.
    run(&mut e, &format!(r#"{{"cmd":"select","ids":[{}]}}"#, ids[0]));
    run(&mut e, r#"{"cmd":"zOrder","to":"front"}"#);
    let p = e.presentation().unwrap();
    let last = p.slides[0].data.shapes.last().and_then(|n| n.nv()).unwrap();
    assert_eq!(last.id, ids[0]);
    run(&mut e, r#"{"cmd":"zOrder","to":"backward"}"#);
    run(&mut e, r#"{"cmd":"zOrder","to":"back"}"#);
    let p = e.presentation().unwrap();
    let first = p.slides[0].data.shapes[0].nv().unwrap();
    assert_eq!(first.id, ids[0]);
    // Undo everything.
    while run(&mut e, r#"{"cmd":"state"}"#).can_undo {
        run(&mut e, r#"{"cmd":"undo"}"#);
    }
    assert_eq!(pres(&e), before);
}

#[test]
fn insert_text_format_and_delete() {
    let mut e = open("slide-text-bullets.pptx");
    let r = run(
        &mut e,
        r#"{"cmd":"insertTextBox","text":"Xin chào","rect":{"x":40,"y":40,"w":300,"h":40}}"#,
    );
    let id = r.selection[0].id;
    assert_eq!(r.selection[0].text.as_deref(), Some("Xin chào"));
    let r = run(
        &mut e,
        r#"{"cmd":"setText","text":"Xin chào Việt Nam\nDòng hai"}"#,
    );
    assert_eq!(
        r.selection[0].text.as_deref(),
        Some("Xin chào Việt Nam\nDòng hai")
    );
    for f in [
        r#"{"kind":"bold"}"#,
        r#"{"kind":"italic","value":true}"#,
        r#"{"kind":"underline"}"#,
        r#"{"kind":"size","points":28}"#,
        r#"{"kind":"color","hex":"C00000"}"#,
        r#"{"kind":"align","align":"center"}"#,
        r#"{"kind":"bullets"}"#,
    ] {
        run(&mut e, &format!(r#"{{"cmd":"formatText","format":{f}}}"#));
    }
    let r = run(&mut e, r#"{"cmd":"state"}"#);
    let f = &r.format;
    assert!(f.bold && f.italic && f.underline && f.bullets, "{f:?}");
    assert_eq!(f.size, Some(28.0));
    assert_eq!(f.color.as_deref(), Some("C00000"));
    run(&mut e, r#"{"cmd":"setFill","color":"FFFF00"}"#);
    run(&mut e, r#"{"cmd":"setLine","color":"0000FF","width":2}"#);
    // Shapes, table and duplicate.
    for preset in ["rect", "ellipse", "rightArrow", "triangle"] {
        let r = run(
            &mut e,
            &format!(r#"{{"cmd":"insertShape","preset":"{preset}"}}"#),
        );
        assert_eq!(r.selection[0].kind, "shape");
    }
    let bad = e
        .slide_command(serde_json::from_str(r#"{"cmd":"insertShape","preset":"nope"}"#).unwrap())
        .unwrap();
    assert_eq!(bad.error.as_deref(), Some("unknownPreset"));
    let r = run(&mut e, r#"{"cmd":"insertTable","rows":3,"cols":4}"#);
    assert_eq!(r.selection[0].kind, "table");
    let r = run(&mut e, r#"{"cmd":"duplicate"}"#);
    assert_ne!(r.selection[0].id, id);
    // The deck saves and reads back with the new objects.
    let bytes = e.save_pptx().unwrap();
    let back = bo_pptx::read_pptx(&bytes).unwrap();
    let mut texts = Vec::new();
    bo_pptx::visit(&back.slides[0].data.shapes, &mut |n| {
        if let ShapeNode::Shape(s) = n {
            if let Some(t) = &s.text {
                texts.push(t.text());
            }
        }
    });
    assert!(texts.iter().any(|t| t == "Xin chào Việt Nam\nDòng hai"));
    // Delete the selection.
    let n = e.presentation().unwrap().slides[0].data.shapes.len();
    run(&mut e, r#"{"cmd":"delete"}"#);
    assert_eq!(e.presentation().unwrap().slides[0].data.shapes.len(), n - 1);
}

#[test]
fn slides_and_notes() {
    let mut e = open("slide-text-bullets.pptx");
    let before = pres(&e);
    let n = before.slides.len();
    let r = run(&mut e, r#"{"cmd":"addSlide"}"#);
    assert!(r.slides_changed);
    assert_eq!(r.info.page_count, n + 1);
    assert_eq!(r.slide, 1);
    assert!(
        !r.placeholders.is_empty(),
        "new slide shows its placeholders"
    );
    let r = run(&mut e, r#"{"cmd":"duplicateSlide","index":0}"#);
    assert_eq!(r.info.page_count, n + 2);
    run(&mut e, r#"{"cmd":"moveSlide","from":0,"to":2}"#);
    let r = run(
        &mut e,
        r#"{"cmd":"setNotes","slide":2,"text":"Ghi chú mới\nDòng 2"}"#,
    );
    assert_eq!(r.notes, "Ghi chú mới\nDòng 2");
    // The deck had no notes master: one is created and the file stays valid OPC.
    let bytes = e.save_pptx().unwrap();
    let pkg = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
    let ct = pkg.content_types().unwrap();
    let nm = pkg
        .parts()
        .iter()
        .find(|p| p.name.contains("/notesMasters/notesMaster"))
        .expect("notes master part");
    assert!(ct.get(&nm.name).unwrap().contains("notesMaster"));
    let back = bo_pptx::read_pptx(&bytes).unwrap();
    assert_eq!(back.slides.len(), n + 2);
    assert_eq!(
        back.slides[2].notes.as_ref().unwrap().text(),
        "Ghi chú mới\nDòng 2"
    );
    let r = run(&mut e, r#"{"cmd":"deleteSlide","index":1}"#);
    assert_eq!(r.info.page_count, n + 1);
    while run(&mut e, r#"{"cmd":"state"}"#).can_undo {
        run(&mut e, r#"{"cmd":"undo"}"#);
    }
    assert_eq!(pres(&e), before);
}

#[test]
fn insert_picture_adds_media_part() {
    let mut e = open("slide-shapes-groups.pptx");
    // 4×2 RGB PNG.
    let mut png = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png, 4, 2);
        enc.set_color(png::ColorType::Rgb);
        let mut w = enc.write_header().unwrap();
        w.write_image_data(&[200u8; 4 * 2 * 3]).unwrap();
    }
    let r = e.slide_insert_picture(png, "a.png").unwrap();
    assert!(r.error.is_none(), "{:?}", r.error);
    assert_eq!(r.selection[0].kind, "picture");
    assert!(r.selection[0].lock_aspect);
    let bytes = e.save_pptx().unwrap();
    let pkg = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).unwrap();
    let ct = pkg.content_types().unwrap();
    let media = pkg
        .parts()
        .iter()
        .find(|p| p.name.starts_with("/ppt/media/image"))
        .expect("media part");
    assert_eq!(ct.get(&media.name), Some("image/png"));
    let mut e2 = Engine::with_bundled_fonts();
    e2.open(&bytes, "x.pptx").unwrap();
    let page = e2.slide(0).unwrap();
    assert!(page
        .commands
        .iter()
        .any(|c| matches!(c, bo_render::Command::Image { .. })));
    run(&mut e, r#"{"cmd":"undo"}"#);
    let after_undo = bo_pptx::read_pptx(&e.save_pptx().unwrap()).unwrap();
    assert!(after_undo.slides[0]
        .info
        .rels
        .iter()
        .all(|r| !r.target.contains("media/image")));
}

// ---------------------------------------------------------------------------
// Property test: random edits, then undo everything == original.
// ---------------------------------------------------------------------------

fn command() -> impl Strategy<Value = String> {
    let pt = || (0.0f64..960.0, 0.0f64..540.0);
    prop_oneof![
        pt().prop_map(|(x, y)| format!(r#"{{"cmd":"click","x":{x},"y":{y}}}"#)),
        pt().prop_map(|(x, y)| format!(r#"{{"cmd":"click","x":{x},"y":{y},"extend":true}}"#)),
        Just(r#"{"cmd":"selectAll"}"#.to_owned()),
        (-50.0f64..50.0, -50.0f64..50.0, any::<bool>()).prop_map(|(dx, dy, s)| format!(
            r#"{{"cmd":"transform","transform":{{"kind":"move","dx":{dx},"dy":{dy}}},"snap":{s}}}"#
        )),
        (0usize..8, -40.0f64..40.0, -40.0f64..40.0, any::<bool>()).prop_map(|(h, dx, dy, k)| {
            let handle = ["nw", "n", "ne", "e", "se", "s", "sw", "w"][h];
            format!(
                r#"{{"cmd":"transform","transform":{{"kind":"resize","handle":"{handle}","dx":{dx},"dy":{dy},"keepAspect":{k}}}}}"#
            )
        }),
        pt().prop_map(|(x, y)| format!(
            r#"{{"cmd":"transform","transform":{{"kind":"rotate","x":{x},"y":{y}}}}}"#
        )),
        Just(r#"{"cmd":"delete"}"#.to_owned()),
        Just(r#"{"cmd":"duplicate"}"#.to_owned()),
        prop::sample::select(vec!["front", "back", "forward", "backward"])
            .prop_map(|z| format!(r#"{{"cmd":"zOrder","to":"{z}"}}"#)),
        Just(r#"{"cmd":"group"}"#.to_owned()),
        Just(r#"{"cmd":"ungroup"}"#.to_owned()),
        prop::sample::select(vec!["left", "center", "right", "top", "middle", "bottom"])
            .prop_map(|a| format!(r#"{{"cmd":"align","to":"{a}"}}"#)),
        prop::sample::select(vec!["horizontal", "vertical"])
            .prop_map(|a| format!(r#"{{"cmd":"distribute","axis":"{a}"}}"#)),
        prop::sample::select(vec!["rect", "ellipse", "rightArrow", "star5"])
            .prop_map(|p| format!(r#"{{"cmd":"insertShape","preset":"{p}"}}"#)),
        "[a-zA-Zàáạảãâầấậẩẫăằắặẳẵ \n]{0,12}"
            .prop_map(|t| format!(r#"{{"cmd":"insertTextBox","text":{}}}"#, serde_json::json!(t))),
        "[a-zđêôơư \n]{0,12}"
            .prop_map(|t| format!(r#"{{"cmd":"setText","text":{}}}"#, serde_json::json!(t))),
        (1usize..4, 1usize..4)
            .prop_map(|(r, c)| format!(r#"{{"cmd":"insertTable","rows":{r},"cols":{c}}}"#)),
        prop::sample::select(vec![
            r#"{"kind":"bold"}"#,
            r#"{"kind":"italic"}"#,
            r#"{"kind":"sizeBy","points":2}"#,
            r#"{"kind":"color","hex":"336699"}"#,
            r#"{"kind":"align","align":"right"}"#,
            r#"{"kind":"bullets"}"#,
        ])
        .prop_map(|f| format!(r#"{{"cmd":"formatText","format":{f}}}"#)),
        Just(r#"{"cmd":"setFill","color":"00AA00"}"#.to_owned()),
        Just(r#"{"cmd":"setLine","color":null}"#.to_owned()),
        Just(r#"{"cmd":"addSlide"}"#.to_owned()),
        Just(r#"{"cmd":"duplicateSlide"}"#.to_owned()),
        Just(r#"{"cmd":"deleteSlide"}"#.to_owned()),
        (0usize..4, 0usize..4)
            .prop_map(|(a, b)| format!(r#"{{"cmd":"moveSlide","from":{a},"to":{b}}}"#)),
        (0usize..4).prop_map(|s| format!(r#"{{"cmd":"goTo","slide":{s}}}"#)),
        "[a-z ]{0,10}"
            .prop_map(|t| format!(r#"{{"cmd":"setNotes","text":{}}}"#, serde_json::json!(t))),
        Just(r#"{"cmd":"undo"}"#.to_owned()),
        Just(r#"{"cmd":"redo"}"#.to_owned()),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, ..ProptestConfig::default() })]

    #[test]
    fn random_edits_then_undo_all_restore_the_deck(
        deck_ix in 0usize..3,
        cmds in prop::collection::vec(command(), 1..14),
    ) {
        let name = ["slide-shapes-groups.pptx", "slide-text-bullets.pptx", "slide-notes-basic.pptx"][deck_ix];
        let mut e = open(name);
        let original = pres(&e);
        for c in &cmds {
            let cmd: SlideCommand = serde_json::from_str(c).unwrap();
            // Refused commands (no selection, …) leave the deck unchanged.
            let r = e.slide_command(cmd).unwrap();
            prop_assert!(r.info.page_count == e.presentation().unwrap().slides.len());
        }
        let edited = pres(&e);
        // Saving always works and reads back.
        let bytes = e.save_pptx().unwrap();
        prop_assert!(bo_pptx::read_pptx(&bytes).is_ok());
        let mut undos = 0;
        while e.slide_command(SlideCommand::State).unwrap().can_undo {
            let r = e.slide_command(SlideCommand::Undo).unwrap();
            prop_assert!(r.error.is_none(), "{:?}", r.error);
            undos += 1;
        }
        prop_assert_eq!(&pres(&e), &original);
        for _ in 0..undos {
            e.slide_command(SlideCommand::Redo).unwrap();
        }
        prop_assert_eq!(&pres(&e), &edited);
    }
}
