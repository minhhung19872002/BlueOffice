//! BlueSlide editing, second part (Phase 4): rich text inside shapes, the
//! clipboard, marquee selection, adjust handles, connectors, table cells and
//! ungrouping flipped groups.

use bo_engine::slide_edit::{SlideCommand, SlideEditResult};
use bo_engine::Engine;
use bo_model::slide::{Autofit, Bullet, ShapeNode, Spacing, TextRun};
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

fn shape(e: &Engine, slide: usize, id: u32) -> &bo_model::slide::Shape {
    let p = e.presentation().unwrap();
    find_shape(&p.slides[slide].data.shapes, id).expect("shape")
}

fn find_shape(nodes: &[ShapeNode], id: u32) -> Option<&bo_model::slide::Shape> {
    for n in nodes {
        match n {
            ShapeNode::Shape(s) if s.nv.id == id => return Some(s),
            ShapeNode::Group(g) => {
                if let Some(s) = find_shape(&g.children, id) {
                    return Some(s);
                }
            }
            _ => {}
        }
    }
    None
}

fn text_box(e: &mut Engine, text: &str) -> u32 {
    let r = run(
        e,
        &format!(
            r#"{{"cmd":"insertTextBox","text":{},"rect":{{"x":100,"y":100,"w":300,"h":40}}}}"#,
            serde_json::json!(text)
        ),
    );
    r.selection[0].id
}

#[test]
fn bold_part_of_a_shape_text() {
    let mut e = open("slide-shapes-presets.pptx");
    let id = text_box(&mut e, "Xin chào Việt Nam");
    // Select "chào" (UTF-16 offsets 4..8) and make it bold.
    let r = run(
        &mut e,
        &format!(r#"{{"cmd":"textSelect","id":{id},"start":4,"end":8}}"#),
    );
    let te = r.text_edit.expect("text editing state");
    assert_eq!((te.start, te.end), (4, 8));
    assert!(te.caret.is_none());
    assert_eq!(te.rects.len(), 1, "one highlighted line");
    let r = run(&mut e, r#"{"cmd":"formatText","format":{"kind":"bold"}}"#);
    assert!(r.format.bold, "state at the selection");
    let s = shape(&e, 0, id);
    let runs: Vec<(String, Option<bool>)> = s.text.as_ref().unwrap().paragraphs[0]
        .runs
        .iter()
        .filter_map(|r| match r {
            TextRun::Run { props, text } => Some((text.clone(), props.b)),
            _ => None,
        })
        .collect();
    assert_eq!(
        runs,
        vec![
            ("Xin ".to_owned(), None),
            ("chào".to_owned(), Some(true)),
            (" Việt Nam".to_owned(), None)
        ]
    );
    // Caret in the plain part: not bold; italic + strike + font + highlight on a range.
    let r = run(
        &mut e,
        &format!(r#"{{"cmd":"textSelect","id":{id},"start":1,"end":1}}"#),
    );
    assert!(!r.format.bold);
    assert!(r.text_edit.unwrap().caret.is_some());
    for f in [
        r#"{"kind":"italic"}"#,
        r#"{"kind":"strike"}"#,
        r#"{"kind":"font","typeface":"Arial"}"#,
        r#"{"kind":"highlight","hex":"FFFF00"}"#,
        r#"{"kind":"color","hex":"C00000"}"#,
        r#"{"kind":"size","points":28}"#,
    ] {
        run(
            &mut e,
            &format!(r#"{{"cmd":"formatText","format":{f},"range":[9,13]}}"#),
        );
    }
    let r = run(
        &mut e,
        &format!(r#"{{"cmd":"textSelect","id":{id},"start":9,"end":13}}"#),
    );
    let f = &r.format;
    assert!(f.italic && f.strike && !f.bold);
    assert_eq!(f.font.as_deref(), Some("Arial"));
    assert_eq!(f.highlight.as_deref(), Some("FFFF00"));
    assert_eq!(f.color.as_deref(), Some("C00000"));
    assert_eq!(f.size, Some(28.0));
    // Saved and reopened: the runs survive.
    let bytes = e.save_pptx().unwrap();
    let mut e2 = Engine::with_bundled_fonts();
    e2.open(&bytes, "x.pptx").unwrap();
    let s2 = shape(&e2, 0, id);
    assert_eq!(s2.text, shape(&e, 0, id).text);
    // Undo restores the plain text runs.
    for _ in 0..7 {
        run(&mut e, r#"{"cmd":"undo"}"#);
    }
    let s = shape(&e, 0, id);
    assert_eq!(s.text.as_ref().unwrap().paragraphs[0].runs.len(), 1);
}

#[test]
fn paragraph_formats_levels_and_spacing() {
    let mut e = open("slide-shapes-presets.pptx");
    let id = text_box(&mut e, "Một\nHai\nBa");
    // Caret in "Hai": bullets, level up (Tab), centre, line spacing 1.5.
    run(
        &mut e,
        &format!(r#"{{"cmd":"textSelect","id":{id},"start":5,"end":5}}"#),
    );
    for f in [
        r#"{"kind":"bullets"}"#,
        r#"{"kind":"level","delta":1}"#,
        r#"{"kind":"align","align":"center"}"#,
        r#"{"kind":"lineSpacing","value":1.5}"#,
    ] {
        run(&mut e, &format!(r#"{{"cmd":"formatText","format":{f}}}"#));
    }
    let r = run(&mut e, r#"{"cmd":"state"}"#);
    assert!(r.format.bullets);
    assert_eq!(r.format.level, 1);
    assert_eq!(r.format.line_spacing, Some(1.5));
    let s = shape(&e, 0, id);
    let ps = &s.text.as_ref().unwrap().paragraphs;
    assert!(ps[0].props.is_none() || ps[0].props.as_ref().unwrap().bullet.is_none());
    let pp = ps[1].props.as_ref().unwrap();
    assert_eq!(pp.bullet, Some(Bullet::Char("•".into())));
    assert_eq!(pp.lvl, Some(1));
    assert_eq!(pp.mar_l, Some(2 * 342_900));
    assert_eq!(pp.algn.as_deref(), Some("ctr"));
    assert_eq!(pp.ln_spc, Some(Spacing::Percent(150_000)));
    // Numbering over the two last paragraphs; Shift+Tab back to level 0.
    run(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"numbering"},"range":[4,10]}"#,
    );
    run(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"level","delta":-1},"range":[4,10]}"#,
    );
    let s = shape(&e, 0, id);
    let ps = &s.text.as_ref().unwrap().paragraphs;
    for p in &ps[1..] {
        let pp = p.props.as_ref().unwrap();
        assert!(matches!(pp.bullet, Some(Bullet::AutoNum { .. })), "{pp:?}");
        assert_eq!(pp.lvl, None);
    }
}

#[test]
fn typing_merges_into_one_undo_step_and_text_box_grows() {
    let mut e = open("slide-shapes-presets.pptx");
    let id = text_box(&mut e, "");
    let h0 = run(&mut e, r#"{"cmd":"state"}"#).selection[0].rect.h;
    let mut text = String::new();
    for word in ["Dòng một", "\nDòng hai", "\nDòng ba", "\nDòng bốn"] {
        text.push_str(word);
        let n = text.encode_utf16().count();
        let r = run(
            &mut e,
            &format!(
                r#"{{"cmd":"setText","id":{id},"text":{},"merge":true,"sel":[{n},{n}]}}"#,
                serde_json::json!(text)
            ),
        );
        let te = r.text_edit.expect("editing");
        assert_eq!((te.start, te.end), (n, n));
        assert!(te.caret.is_some());
    }
    // spAutoFit: the box grew with four lines.
    let r = run(&mut e, r#"{"cmd":"state"}"#);
    assert!(
        r.selection[0].rect.h > h0 * 2.0,
        "{} vs {h0}",
        r.selection[0].rect.h
    );
    // The caret sits on the last line, below the first.
    let te = r.text_edit.unwrap();
    let c = te.caret.unwrap();
    assert!(c[1] > te.rect.cy, "{c:?}");
    // Clicking the first line puts the caret there.
    let top = te.rect.cy - te.rect.h / 2.0 + 10.0;
    let r = run(
        &mut e,
        &format!(
            r#"{{"cmd":"textHit","x":{},"y":{top}}}"#,
            te.rect.cx - te.rect.w / 2.0 + 2.0
        ),
    );
    assert_eq!(r.text_edit.unwrap().start, 0);
    // One undo removes all the typing (and the growth).
    run(&mut e, r#"{"cmd":"undo"}"#);
    let s = shape(&e, 0, id);
    assert_eq!(s.text.as_ref().unwrap().text(), "");
    let r = run(&mut e, r#"{"cmd":"textExit"}"#);
    assert!(r.text_edit.is_none());
    assert!((r.selection[0].rect.h - h0).abs() < 0.01);
}

#[test]
fn shrink_on_overflow_records_font_scale() {
    let mut e = open("slide-shapes-presets.pptx");
    let id = text_box(&mut e, "");
    // Turn the text box into a fixed box that shrinks text on overflow.
    {
        let p = e.presentation_mut().unwrap();
        for n in &mut p.slides[0].data.shapes {
            if let ShapeNode::Shape(s) = n {
                if s.nv.id == id {
                    s.text.as_mut().unwrap().body_pr.autofit = Some(Autofit::Normal {
                        font_scale: None,
                        ln_spc_reduction: None,
                    });
                }
            }
        }
    }
    let long = "Văn bản rất dài ".repeat(30);
    run(
        &mut e,
        &format!(
            r#"{{"cmd":"setText","id":{id},"text":{}}}"#,
            serde_json::json!(long)
        ),
    );
    let s = shape(&e, 0, id);
    match &s.text.as_ref().unwrap().body_pr.autofit {
        Some(Autofit::Normal {
            font_scale: Some(f),
            ..
        }) => assert!(*f < 100_000),
        other => panic!("{other:?}"),
    }
}

fn first_picture(e: &Engine, slide: usize) -> (u32, String) {
    let p = e.presentation().unwrap();
    p.slides[slide]
        .data
        .shapes
        .iter()
        .find_map(|n| match n {
            ShapeNode::Picture(pic) => Some((pic.nv.id, pic.blip.embed.clone().unwrap())),
            _ => None,
        })
        .expect("a picture")
}

fn pictures(e: &Engine, slide: usize) -> Vec<(u32, String, (i64, i64))> {
    let p = e.presentation().unwrap();
    p.slides[slide]
        .data
        .shapes
        .iter()
        .filter_map(|n| match n {
            ShapeNode::Picture(pic) => Some((
                pic.nv.id,
                pic.blip.embed.clone().unwrap(),
                pic.props.xfrm.as_ref().unwrap().off.unwrap(),
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn copy_paste_shapes_with_pictures_across_slides_and_decks() {
    let mut e = open("slide-images-crop.pptx");
    let (id, rid) = first_picture(&e, 0);
    run(&mut e, &format!(r#"{{"cmd":"select","ids":[{id}]}}"#));
    let r = run(&mut e, r#"{"cmd":"copy"}"#);
    let clip = r.clipboard.expect("system clipboard content");
    assert!(clip.png.as_deref().is_some_and(|p| p.len() > 100));
    assert_eq!(r.clip, Some("shapes"));
    // Same slide: a copy, offset by 12 pt, same image relationship.
    let before = pictures(&e, 0);
    let r = run(&mut e, r#"{"cmd":"paste"}"#);
    let after = pictures(&e, 0);
    assert_eq!(after.len(), before.len() + 1);
    let new = after.iter().find(|p| p.0 == r.selection[0].id).unwrap();
    let orig = before.iter().find(|p| p.0 == id).unwrap();
    assert_ne!(new.0, id);
    assert_eq!(new.1, rid);
    assert_eq!(new.2, (orig.2 .0 + 152_400, orig.2 .1 + 152_400));
    // Another slide: no offset, a relationship to the same media part.
    let n = e.presentation().unwrap().slides.len();
    let target = if n > 1 {
        1
    } else {
        run(&mut e, r#"{"cmd":"addSlide"}"#).slide
    };
    run(&mut e, &format!(r#"{{"cmd":"goTo","slide":{target}}}"#));
    run(&mut e, r#"{"cmd":"paste"}"#);
    let pasted = pictures(&e, target);
    let p = e.presentation().unwrap();
    let rel = p.slides[target]
        .info
        .rels
        .iter()
        .find(|r| r.id == pasted.last().unwrap().1)
        .unwrap();
    let src_rel = p.slides[0].info.rels.iter().find(|r| r.id == rid).unwrap();
    assert_eq!(rel.target, src_rel.target);
    assert_eq!(pasted.last().unwrap().2, orig.2);
    let parts = p.parts.len();
    // Another deck in the same engine: the image part comes along.
    e.open(&deck("slide-shapes-presets.pptx"), "b.pptx")
        .unwrap();
    let r = run(&mut e, r#"{"cmd":"paste"}"#);
    assert_eq!(r.selection.len(), 1);
    let (pid, prid) = first_picture(&e, 0);
    assert_eq!(pid, r.selection[0].id);
    let p = e.presentation().unwrap();
    let rel = p.slides[0].info.rels.iter().find(|r| r.id == prid).unwrap();
    let part = bo_opc::resolve_target("/ppt/slides/slide1.xml", &rel.target);
    assert!(p.part_data(&part).is_some(), "{part}");
    let _ = parts;
    // Saved and reopened, the picture is there.
    let bytes = e.save_pptx().unwrap();
    let mut e2 = Engine::with_bundled_fonts();
    e2.open(&bytes, "c.pptx").unwrap();
    assert_eq!(first_picture(&e2, 0).0, pid);
    // Cut removes, paste brings it back.
    run(&mut e, &format!(r#"{{"cmd":"select","ids":[{pid}]}}"#));
    run(&mut e, r#"{"cmd":"cut"}"#);
    assert_eq!(pictures(&e, 0).len(), 0);
    run(&mut e, r#"{"cmd":"paste"}"#);
    assert_eq!(pictures(&e, 0).len(), 1);
}

#[test]
fn copy_cut_paste_slides() {
    let mut e = open("slide-notes-basic.pptx");
    let n = e.presentation().unwrap().slides.len();
    let title = |e: &Engine, i: usize| {
        bo_engine::slide_edit::clip::plain_text(&e.presentation().unwrap().slides[i].data.shapes)
    };
    let t0 = title(&e, 0);
    let r = run(&mut e, r#"{"cmd":"copySlides","indices":[0]}"#);
    assert_eq!(r.clip, Some("slides"));
    assert_eq!(r.clipboard.unwrap().text, t0);
    run(&mut e, &format!(r#"{{"cmd":"goTo","slide":{}}}"#, n - 1));
    let r = run(&mut e, r#"{"cmd":"paste"}"#);
    assert_eq!(r.slide, n);
    assert_eq!(e.presentation().unwrap().slides.len(), n + 1);
    assert_eq!(title(&e, n), t0);
    // Part names stay unique.
    let p = e.presentation().unwrap();
    let mut names: Vec<&str> = p.slides.iter().map(|s| s.info.part_name.as_str()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), n + 1);
    run(&mut e, &format!(r#"{{"cmd":"cutSlides","indices":[{n}]}}"#));
    assert_eq!(e.presentation().unwrap().slides.len(), n);
    run(&mut e, r#"{"cmd":"goTo","slide":0}"#);
    run(&mut e, r#"{"cmd":"paste"}"#);
    assert_eq!(title(&e, 1), t0);
    let bytes = e.save_pptx().unwrap();
    assert_eq!(bo_pptx::read_pptx(&bytes).unwrap().slides.len(), n + 1);
    // Slides cannot go to another deck.
    e.open(&deck("slide-shapes-presets.pptx"), "b.pptx")
        .unwrap();
    let r = try_run(&mut e, r#"{"cmd":"paste"}"#);
    assert_eq!(r.error.as_deref(), Some("clipOtherDeck"));
}

#[test]
fn marquee_select_all_and_tab_cycle() {
    let mut e = open("slide-shapes-presets.pptx");
    let all = run(&mut e, r#"{"cmd":"selectAll"}"#).selection;
    assert!(all.len() > 3);
    // A box around the first two shapes selects exactly those inside it.
    let b0 = all[0].rect;
    let b1 = all[1].rect;
    let x0 = (b0.cx - b0.w / 2.0).min(b1.cx - b1.w / 2.0) - 1.0;
    let y0 = (b0.cy - b0.h / 2.0).min(b1.cy - b1.h / 2.0) - 1.0;
    let x1 = (b0.cx + b0.w / 2.0).max(b1.cx + b1.w / 2.0) + 1.0;
    let y1 = (b0.cy + b0.h / 2.0).max(b1.cy + b1.h / 2.0) + 1.0;
    let r = run(
        &mut e,
        &format!(
            r#"{{"cmd":"selectRect","rect":{{"x":{x0},"y":{y0},"w":{},"h":{}}}}}"#,
            x1 - x0,
            y1 - y0
        ),
    );
    let ids: Vec<u32> = r.selection.iter().map(|s| s.id).collect();
    assert!(
        ids.contains(&all[0].id) && ids.contains(&all[1].id),
        "{ids:?}"
    );
    // Empty box: nothing; extend keeps the selection.
    let r = run(
        &mut e,
        r#"{"cmd":"selectRect","rect":{"x":0,"y":0,"w":1,"h":1},"extend":true}"#,
    );
    assert_eq!(r.selection.len(), ids.len());
    let r = run(
        &mut e,
        r#"{"cmd":"selectRect","rect":{"x":0,"y":0,"w":1,"h":1}}"#,
    );
    assert_eq!(r.selection.len(), 0);
    // Tab cycles in z-order, Shift+Tab goes back and wraps.
    let r = run(&mut e, r#"{"cmd":"selectNext"}"#);
    assert_eq!(r.selection[0].id, all[0].id);
    let r = run(&mut e, r#"{"cmd":"selectNext"}"#);
    assert_eq!(r.selection[0].id, all[1].id);
    run(&mut e, r#"{"cmd":"selectNext","back":true}"#);
    let r = run(&mut e, r#"{"cmd":"selectNext","back":true}"#);
    assert_eq!(r.selection[0].id, all.last().unwrap().id);
}

#[test]
fn adjust_handles_change_the_preset() {
    let mut e = open("slide-shapes-presets.pptx");
    let r = run(
        &mut e,
        r#"{"cmd":"insertShape","preset":"roundRect","rect":{"x":100,"y":100,"w":200,"h":100}}"#,
    );
    let id = r.selection[0].id;
    assert_eq!(r.adjust.len(), 1);
    let h = r.adjust[0];
    assert!((h.y - 100.0).abs() < 1e-6, "on the top edge: {h:?}");
    // Drag the handle 10 pt to the right twice: one undo step.
    run(
        &mut e,
        &format!(
            r#"{{"cmd":"adjust","index":0,"x":{},"y":100,"merge":true}}"#,
            h.x + 5.0
        ),
    );
    let r = run(
        &mut e,
        &format!(
            r#"{{"cmd":"adjust","index":0,"x":{},"y":100,"merge":true}}"#,
            h.x + 10.0
        ),
    );
    assert!((r.adjust[0].x - (h.x + 10.0)).abs() < 0.5, "{:?}", r.adjust);
    let s = shape(&e, 0, id);
    match &s.props.geometry {
        Some(bo_model::slide::Geometry::Preset { name, av }) => {
            assert_eq!(name, "roundRect");
            assert_eq!(av.len(), 1);
            assert_eq!(av[0].name, "adj");
        }
        g => panic!("{g:?}"),
    }
    run(&mut e, r#"{"cmd":"undo"}"#);
    let r = run(&mut e, &format!(r#"{{"cmd":"select","ids":[{id}]}}"#));
    assert!((r.adjust[0].x - h.x).abs() < 1e-6);
}

fn connector_of(e: &Engine, id: u32) -> bo_model::slide::Connector {
    e.presentation().unwrap().slides[0]
        .data
        .shapes
        .iter()
        .find_map(|n| match n {
            ShapeNode::Connector(c) if c.nv.id == id => Some((**c).clone()),
            _ => None,
        })
        .expect("connector")
}

#[test]
fn connectors_attach_to_sites_and_follow_shapes() {
    use bo_engine::slide_edit::connect::cxn_ref;
    let mut e = open("slide-shapes-presets.pptx");
    let a = run(
        &mut e,
        r#"{"cmd":"insertShape","preset":"rect","rect":{"x":100,"y":300,"w":100,"h":60}}"#,
    )
    .selection[0]
        .id;
    let b = run(
        &mut e,
        r#"{"cmd":"insertShape","preset":"ellipse","rect":{"x":400,"y":300,"w":100,"h":60}}"#,
    )
    .selection[0]
        .id;
    run(&mut e, &format!(r#"{{"cmd":"select","ids":[{a},{b}]}}"#));
    let r = run(&mut e, r#"{"cmd":"insertConnector"}"#);
    let cid = r.selection[0].id;
    let conn = r.connector.expect("connector state");
    assert_eq!(conn.attached, [true, true]);
    // Right edge of the rectangle to the left of the ellipse.
    assert!((conn.ends[0][0] - 200.0).abs() < 0.01 && (conn.ends[0][1] - 330.0).abs() < 0.01);
    assert!((conn.ends[1][0] - 400.0).abs() < 0.01, "{conn:?}");
    assert!(r.sites.len() > 4);
    let c = connector_of(&e, cid);
    assert_eq!(cxn_ref(&c, 0).map(|r| r.0), Some(a));
    assert_eq!(cxn_ref(&c, 1).map(|r| r.0), Some(b));
    // Moving the ellipse down re-routes the connector (flipped box when going up).
    run(&mut e, &format!(r#"{{"cmd":"select","ids":[{b}]}}"#));
    run(
        &mut e,
        r#"{"cmd":"transform","transform":{"kind":"move","dx":0,"dy":-100},"snap":false}"#,
    );
    let r = run(&mut e, &format!(r#"{{"cmd":"select","ids":[{cid}]}}"#));
    let conn = r.connector.unwrap();
    assert!((conn.ends[1][1] - 230.0).abs() < 0.01, "{conn:?}");
    assert!((conn.ends[0][1] - 330.0).abs() < 0.01, "{conn:?}");
    assert_eq!(connector_of(&e, cid).props.xfrm.unwrap().flip_v, Some(true));
    // Dragging the end away detaches it; dropping it near a site attaches it again.
    let r = run(
        &mut e,
        r#"{"cmd":"connectorEnd","end":1,"x":600,"y":100,"merge":true}"#,
    );
    assert_eq!(r.connector.unwrap().attached, [true, false]);
    let r = run(
        &mut e,
        r#"{"cmd":"connectorEnd","end":1,"x":452,"y":202,"merge":true}"#,
    );
    let conn = r.connector.unwrap();
    assert_eq!(conn.attached, [true, true]);
    assert!((conn.ends[1][0] - 450.0).abs() < 0.01 && (conn.ends[1][1] - 200.0).abs() < 0.01);
    // Saved and reopened, the connection survives.
    let bytes = e.save_pptx().unwrap();
    let p = bo_pptx::read_pptx(&bytes).unwrap();
    let c = p.slides[0]
        .data
        .shapes
        .iter()
        .find_map(|n| match n {
            ShapeNode::Connector(c) if c.nv.id == cid => Some(c.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(cxn_ref(&c, 0).map(|r| r.0), Some(a));
    assert_eq!(cxn_ref(&c, 1).map(|r| r.0), Some(b));
}

fn table_of(e: &Engine, id: u32) -> bo_model::slide::Table {
    e.presentation().unwrap().slides[0]
        .data
        .shapes
        .iter()
        .find_map(|n| match n {
            ShapeNode::Frame(f) if f.nv.id == id => match &f.content {
                bo_model::slide::GraphicContent::Table(t) => Some(t.clone()),
                bo_model::slide::GraphicContent::Other(_) => None,
            },
            _ => None,
        })
        .expect("table")
}

#[test]
fn table_cells_rows_columns_merge_fill_and_style() {
    let mut e = open("slide-shapes-presets.pptx");
    let r = run(
        &mut e,
        r#"{"cmd":"insertTable","rows":3,"cols":3,"rect":{"x":100,"y":100,"w":300,"h":90}}"#,
    );
    let id = r.selection[0].id;
    // The built-in style is written to the deck's table styles part.
    let styles = e
        .presentation()
        .unwrap()
        .part_data(bo_pptx::table_style::TABLE_STYLES_PART)
        .map(|d| String::from_utf8_lossy(d).into_owned())
        .unwrap();
    assert!(
        styles.contains(bo_pptx::table_style::BUILTIN_IDS[0]),
        "{styles}"
    );
    // Click a cell and type.
    let r = run(&mut e, r#"{"cmd":"tableHit","x":150,"y":140}"#);
    let t = r.table.expect("table state");
    assert_eq!((t.rows, t.cols, t.sel), (3, 3, [1, 0, 1, 0]));
    assert_eq!(t.cells.len(), 9);
    for (i, text) in ["H", "Hà", "Hà Nội"].iter().enumerate() {
        let _ = i;
        run(
            &mut e,
            &format!(r#"{{"cmd":"setCellText","text":"{text}","merge":true}}"#),
        );
    }
    let tb = table_of(&e, id);
    assert_eq!(tb.rows[1].cells[0].text.as_ref().unwrap().text(), "Hà Nội");
    // Rows and columns grow the frame.
    run(&mut e, r#"{"cmd":"tableInsertRow","below":true}"#);
    let r = run(&mut e, r#"{"cmd":"tableInsertCol","right":true}"#);
    let t = r.table.unwrap();
    assert_eq!((t.rows, t.cols), (4, 4));
    assert!(
        (r.selection[0].rect.w - 400.0).abs() < 0.1,
        "{:?}",
        r.selection[0].rect
    );
    // Merge 2×2, then split again.
    run(
        &mut e,
        r#"{"cmd":"tableSelect","r0":1,"c0":0,"r1":2,"c1":1}"#,
    );
    let r = run(&mut e, r#"{"cmd":"tableMerge"}"#);
    assert_eq!(r.table.unwrap().cells.len(), 13);
    let tb = table_of(&e, id);
    assert_eq!(tb.rows[1].cells[0].grid_span, Some(2));
    assert_eq!(tb.rows[1].cells[0].row_span, Some(2));
    let r = run(&mut e, r#"{"cmd":"tableSplit"}"#);
    assert_eq!(r.table.unwrap().cells.len(), 16);
    // Cell fill and style options.
    run(&mut e, r#"{"cmd":"tableFill","color":"FFC000"}"#);
    run(
        &mut e,
        r#"{"cmd":"tableOptions","firstRow":false,"bandRow":false}"#,
    );
    let r = run(&mut e, r#"{"cmd":"state"}"#);
    let t = r.table.unwrap();
    assert!(!t.first_row && !t.band_row);
    let tb = table_of(&e, id);
    assert!(matches!(
        tb.rows[1].cells[0].props.as_ref().unwrap().fill,
        Some(bo_model::slide::Fill::Solid(_))
    ));
    // Delete a row and a column.
    run(&mut e, r#"{"cmd":"tableSelect","r0":3,"c0":3}"#);
    run(&mut e, r#"{"cmd":"tableDeleteRows"}"#);
    let r = run(&mut e, r#"{"cmd":"tableDeleteCols"}"#);
    let t = r.table.unwrap();
    assert_eq!((t.rows, t.cols), (3, 3));
    // Saved and reopened.
    let bytes = e.save_pptx().unwrap();
    let mut e2 = Engine::with_bundled_fonts();
    e2.open(&bytes, "t.pptx").unwrap();
    assert_eq!(table_of(&e2, id), table_of(&e, id));
    // Leaving: another command ends table editing.
    let r = run(&mut e, r#"{"cmd":"select","ids":[]}"#);
    assert!(r.table.is_none());
}

#[test]
fn table_cell_text_edited_on_the_canvas_with_partial_formatting() {
    let mut e = open("slide-shapes-presets.pptx");
    let r = run(
        &mut e,
        r#"{"cmd":"insertTable","rows":2,"cols":2,"rect":{"x":100,"y":100,"w":300,"h":80}}"#,
    );
    let id = r.selection[0].id;
    let b = r.selection[0].rect;
    let (cx, cy) = (b.cx - b.w / 4.0, b.cy + b.h / 4.0);
    // Double-click a cell: its text is edited on the canvas, caret in the cell box.
    let r = run(
        &mut e,
        &format!(r#"{{"cmd":"cellTextHit","x":{cx},"y":{cy}}}"#),
    );
    let te = r.text_edit.expect("text edit");
    assert_eq!(te.cell, Some([1, 0]));
    assert!(te.rect.w > 100.0 && te.rect.w < 200.0, "{:?}", te.rect);
    assert!(te.caret.is_some());
    for t in ["H", "Hà", "Hà Nội"] {
        run(
            &mut e,
            &format!(
                r#"{{"cmd":"setText","id":{id},"text":"{t}","merge":true,"sel":[{n},{n}],"cell":[1,0]}}"#,
                n = t.encode_utf16().count()
            ),
        );
    }
    // Bold "Nội" only (partial formatting inside the cell).
    run(
        &mut e,
        &format!(r#"{{"cmd":"textSelect","id":{id},"start":3,"end":6,"cell":[1,0]}}"#),
    );
    let r = run(&mut e, r#"{"cmd":"formatText","format":{"kind":"bold"}}"#);
    assert!(r.format.bold);
    let te = r.text_edit.expect("still editing");
    assert_eq!(te.rects.len(), 1);
    let tb = table_of(&e, id);
    let runs = &tb.rows[1].cells[0].text.as_ref().unwrap().paragraphs[0].runs;
    let bold: Vec<(String, bool)> = runs
        .iter()
        .filter_map(|r| match r {
            TextRun::Run { props, text } => Some((text.clone(), props.b == Some(true))),
            _ => None,
        })
        .collect();
    assert_eq!(
        bold,
        vec![("Hà ".to_owned(), false), ("Nội".to_owned(), true)]
    );
    // Tab: the next cell, its text selected; Shift+Tab back.
    let r = run(&mut e, r#"{"cmd":"cellNext"}"#);
    assert_eq!(r.text_edit.unwrap().cell, Some([1, 1]));
    let r = run(&mut e, r#"{"cmd":"cellNext","back":true}"#);
    let te = r.text_edit.unwrap();
    assert_eq!((te.cell, te.start, te.end), (Some([1, 0]), 0, 6));
    // Caret by pointer inside the cell.
    let r = run(
        &mut e,
        &format!(
            r#"{{"cmd":"textHit","x":{},"y":{cy}}}"#,
            b.cx - b.w / 2.0 + 1.0
        ),
    );
    assert_eq!(r.text_edit.unwrap().end, 0);
    // One undo step per typing burst; saved and reopened.
    let bytes = e.save_pptx().unwrap();
    let mut e2 = Engine::with_bundled_fonts();
    e2.open(&bytes, "t.pptx").unwrap();
    assert_eq!(table_of(&e2, id), table_of(&e, id));
    run(&mut e, r#"{"cmd":"undo"}"#);
    let tb = table_of(&e, id);
    assert_eq!(tb.rows[1].cells[0].text.as_ref().unwrap().text(), "Hà Nội");
}

/// Points of every path on a slide, rounded (for comparing renderings).
fn path_points(e: &mut Engine, slide: usize) -> Vec<(i64, i64)> {
    let page = e.refresh_slide(slide).unwrap().clone();
    let mut out = Vec::new();
    // Apply pushed transforms so equal drawings compare equal.
    let mut stack = vec![[1.0f64, 0.0, 0.0, 1.0, 0.0, 0.0]];
    for c in &page.commands {
        match c {
            bo_render::Command::PushTransform { m } => {
                let [a, b, cc, d, ee, f] = *stack.last().unwrap();
                let m = m.map(f64::from);
                stack.push([
                    a * m[0] + cc * m[1],
                    b * m[0] + d * m[1],
                    a * m[2] + cc * m[3],
                    b * m[2] + d * m[3],
                    a * m[4] + cc * m[5] + ee,
                    b * m[4] + d * m[5] + f,
                ]);
            }
            bo_render::Command::PopTransform => {
                stack.pop();
            }
            bo_render::Command::Path { segs, .. } => {
                let t = *stack.last().unwrap();
                for s in segs {
                    let pts: Vec<(f32, f32)> = match *s {
                        bo_render::PathSeg::M(x, y) | bo_render::PathSeg::L(x, y) => vec![(x, y)],
                        bo_render::PathSeg::Q(a, b, x, y) => vec![(a, b), (x, y)],
                        bo_render::PathSeg::C(a, b, c2, d, x, y) => vec![(a, b), (c2, d), (x, y)],
                        bo_render::PathSeg::Z => vec![],
                    };
                    for (x, y) in pts {
                        let (x, y) = (f64::from(x), f64::from(y));
                        out.push((
                            ((t[0] * x + t[2] * y + t[4]) * 10.0).round() as i64,
                            ((t[1] * x + t[3] * y + t[5]) * 10.0).round() as i64,
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    out.sort_unstable();
    out
}

#[test]
fn ungroup_keeps_flips() {
    let mut e = open("slide-shapes-presets.pptx");
    run(&mut e, r#"{"cmd":"addSlide"}"#);
    let slide = run(&mut e, r#"{"cmd":"state"}"#).slide;
    let a = run(
        &mut e,
        r#"{"cmd":"insertShape","preset":"rightArrow","rect":{"x":100,"y":100,"w":160,"h":60}}"#,
    )
    .selection[0]
        .id;
    run(
        &mut e,
        r#"{"cmd":"transform","transform":{"kind":"rotate","x":300,"y":60}}"#,
    );
    let b = run(
        &mut e,
        r#"{"cmd":"insertShape","preset":"triangle","rect":{"x":320,"y":200,"w":80,"h":100}}"#,
    )
    .selection[0]
        .id;
    let g = run(&mut e, &format!(r#"{{"cmd":"select","ids":[{a},{b}]}}"#));
    let _ = g;
    let gid = run(&mut e, r#"{"cmd":"group"}"#).selection[0].id;
    for (fh, fv) in [(true, false), (false, true), (true, true)] {
        {
            let p = e.presentation_mut().unwrap();
            for n in &mut p.slides[slide].data.shapes {
                if let ShapeNode::Group(gr) = n {
                    if gr.nv.id == gid {
                        let x = gr.props.xfrm.as_mut().unwrap();
                        x.flip_h = fh.then_some(true);
                        x.flip_v = fv.then_some(true);
                    }
                }
            }
        }
        let before = path_points(&mut e, slide);
        run(&mut e, &format!(r#"{{"cmd":"select","ids":[{gid}]}}"#));
        run(&mut e, r#"{"cmd":"ungroup"}"#);
        let after = path_points(&mut e, slide);
        let worst = before
            .iter()
            .zip(&after)
            .map(|(p, q)| (p.0 - q.0).abs().max((p.1 - q.1).abs()))
            .max()
            .unwrap_or(0);
        assert_eq!(before.len(), after.len());
        assert!(
            worst <= 2,
            "flip {fh}/{fv}: drawing moved by {worst} tenths of a point"
        );
        run(&mut e, r#"{"cmd":"undo"}"#);
    }
}
#[test]
fn blurred_shadows_and_picture_transparency_render() {
    use bo_model::slide::{Color, Effect, OuterShadow, XmlElement};
    let mut e = open("slide-images-crop.pptx");
    let r = run(
        &mut e,
        r#"{"cmd":"insertShape","preset":"rect","rect":{"x":500,"y":300,"w":100,"h":60}}"#,
    );
    let sid = r.selection[0].id;
    {
        let p = e.presentation_mut().unwrap();
        for n in &mut p.slides[0].data.shapes {
            match n {
                ShapeNode::Shape(s) if s.nv.id == sid => {
                    // §20.1.8.49 outerShdw with a 6 pt blur.
                    s.props.effects = Some(vec![Effect::OuterShadow(OuterShadow {
                        blur_rad: Some(76_200),
                        dist: Some(38_100),
                        dir: Some(2_700_000),
                        color: Some(Color::srgb(0)),
                        attrs: vec![],
                    })]);
                }
                ShapeNode::Picture(pic) => {
                    let mut a = XmlElement::new("a:alphaModFix");
                    a.attrs.push(("amt".into(), "40000".into()));
                    pic.blip.blip_children.push(a);
                }
                _ => {}
            }
        }
    }
    let page = e.refresh_slide(0).unwrap().clone();
    assert!(page.commands.iter().any(|c| matches!(
        c,
        bo_render::Command::BlurPath { radius, .. } if (*radius - 6.0).abs() < 1e-3
    )));
    assert!(page.commands.iter().any(|c| matches!(
        c,
        bo_render::Command::PushOpacity { alpha } if (*alpha - 0.4).abs() < 1e-3
    )));
    // Every backend takes the new commands.
    assert!(e.render_png(0, 0.5).unwrap().len() > 100);
    let pdf = e.export_pdf().unwrap();
    assert!(String::from_utf8_lossy(&pdf).contains("/ca 0.4"));
    // The edit survives a save.
    let bytes = e.save_pptx().unwrap();
    let mut e2 = Engine::with_bundled_fonts();
    e2.open(&bytes, "x.pptx").unwrap();
    assert!(e2
        .slide(0)
        .unwrap()
        .commands
        .iter()
        .any(|c| matches!(c, bo_render::Command::PushOpacity { .. })));
}
