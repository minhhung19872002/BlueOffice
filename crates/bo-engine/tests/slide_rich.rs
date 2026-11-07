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
    assert!(r.selection.is_empty());
    // Tab cycles in z-order, Shift+Tab goes back and wraps.
    let r = run(&mut e, r#"{"cmd":"selectNext"}"#);
    assert_eq!(r.selection[0].id, all[0].id);
    let r = run(&mut e, r#"{"cmd":"selectNext"}"#);
    assert_eq!(r.selection[0].id, all[1].id);
    run(&mut e, r#"{"cmd":"selectNext","back":true}"#);
    let r = run(&mut e, r#"{"cmd":"selectNext","back":true}"#);
    assert_eq!(r.selection[0].id, all.last().unwrap().id);
}
