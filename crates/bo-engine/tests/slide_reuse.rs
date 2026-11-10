//! Reuse slides (SLIDE-ALBUM-001): `reuseOpen` lists another deck with
//! thumbnails, `reuseSlides` inserts chosen slides in one undoable step, with
//! or without their source design.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use base64::Engine as _;
use bo_engine::slide_edit::{SlideCommand, SlideEditResult};
use bo_engine::Engine;
use bo_model::slide::ShapeNode;
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

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// `reuseOpen` of `bytes`; the answer as JSON.
fn reuse_open(e: &mut Engine, bytes: &[u8]) -> serde_json::Value {
    let json = format!(
        r#"{{"cmd":"reuseOpen","name":"src.pptx","base64":"{}","width":120}}"#,
        b64(bytes)
    );
    serde_json::from_str(&e.slide_command_json(&json).unwrap()).unwrap()
}

fn pictures(e: &Engine, slide: usize) -> usize {
    let mut n = 0;
    bo_pptx::visit(
        &e.presentation().unwrap().slides[slide].data.shapes,
        &mut |s| {
            if matches!(s, ShapeNode::Picture(_)) {
                n += 1;
            }
        },
    );
    n
}

fn text(p: &bo_model::slide::Presentation, slide: usize) -> String {
    bo_engine::slide_edit::clip::plain_text(&p.slides[slide].data.shapes)
}

/// Every internal relationship targets an existing part; layout / master /
/// theme indices are in range.
fn assert_package_valid(bytes: &[u8]) -> bo_model::slide::Presentation {
    let p = bo_pptx::read_pptx(bytes).unwrap();
    let pkg = bo_opc::Package::read(bytes, &bo_opc::Limits::default()).unwrap();
    let names: Vec<String> = pkg.parts().iter().map(|p| p.name.clone()).collect();
    for name in names {
        let Ok(rels) = pkg.relationships(&name) else {
            continue;
        };
        for r in rels {
            if r.mode == bo_opc::TargetMode::External {
                continue;
            }
            let target = bo_opc::resolve_target(&name, &r.target);
            assert!(
                pkg.part(&target).is_some(),
                "{name}: {} → {target} missing",
                r.id
            );
        }
    }
    for s in &p.slides {
        assert!(s.layout < p.layouts.len());
    }
    for l in &p.layouts {
        assert!(l.master < p.masters.len());
    }
    for m in &p.masters {
        assert!(m.theme < p.themes.len());
    }
    p
}

/// A deck whose layouts match nothing of the corpus decks (other names, no placeholders).
fn bare_target() -> Vec<u8> {
    let mut target = bo_pptx::read_pptx(&deck("slide-shapes-presets.pptx")).unwrap();
    for l in &mut target.layouts {
        l.data.name = Some("Riêng".into());
        l.data.shapes.clear();
    }
    bo_pptx::write_pptx(&target).unwrap()
}

#[test]
fn reuse_open_lists_slides_with_titles_and_thumbnails() {
    let src = deck("slide-text-bullets.pptx");
    let expected = bo_pptx::read_pptx(&src).unwrap();
    let mut e = open("slide-shapes-presets.pptx");
    let r = reuse_open(&mut e, &src);
    assert_eq!(r["reuse"]["name"], "src.pptx");
    let slides = r["reuse"]["slides"].as_array().unwrap();
    assert_eq!(slides.len(), expected.slides.len());
    for (i, s) in slides.iter().enumerate() {
        // The title is the title placeholder's text, as the outline reports it.
        let title = bo_engine::slide_edit::outline::slide_outline(&expected.slides[i]).title;
        assert_eq!(s["title"].as_str().unwrap(), title);
        let png = base64::engine::general_purpose::STANDARD
            .decode(s["png"].as_str().unwrap())
            .unwrap();
        // PNG signature (ISO/IEC 15948, §5.2) and the requested width in IHDR.
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(u32::from_be_bytes([png[16], png[17], png[18], png[19]]), 120);
    }
    // Listing changes nothing in the open deck.
    assert!(!run(&mut e, r#"{"cmd":"state"}"#).can_undo);
}

#[test]
fn reuse_slides_inserts_after_the_current_slide_in_one_undo_step() {
    // Source: a text deck with a pictures slide at index 1.
    let src = deck("slide-edit-paste-other-deck.pptx");
    let src_pres = bo_pptx::read_pptx(&src).unwrap();
    assert!(src_pres.slides.len() >= 3);
    let mut e = open("slide-shapes-presets.pptx");
    let before = e.presentation().unwrap().clone();
    let n = before.slides.len();
    reuse_open(&mut e, &src);
    run(&mut e, r#"{"cmd":"goTo","slide":0}"#);
    let r = run(&mut e, r#"{"cmd":"reuseSlides","indices":[1,2]}"#);
    assert_eq!(r.slide, 1);
    {
        let p = e.presentation().unwrap();
        assert_eq!(p.slides.len(), n + 2);
        // Destination design: no layout, master or theme is added.
        assert_eq!(p.layouts.len(), before.layouts.len());
        assert_eq!(p.masters.len(), before.masters.len());
        assert_eq!(text(p, 1), text(&src_pres, 1));
        assert_eq!(text(p, 2), text(&src_pres, 2));
        assert_eq!(text(p, 3), text(&before, 1));
    }
    let src_pics = {
        let mut s = Engine::with_bundled_fonts();
        s.open(&src, "s.pptx").unwrap();
        (pictures(&s, 1), pictures(&s, 2))
    };
    assert!(src_pics.0 > 0);
    assert_eq!((pictures(&e, 1), pictures(&e, 2)), src_pics);
    // Saved: relationships valid, the slides and their pictures are there.
    let bytes = e.save_pptx().unwrap();
    let saved = assert_package_valid(&bytes);
    assert_eq!(saved.slides.len(), n + 2);
    assert_eq!(text(&saved, 1), text(&src_pres, 1));
    let mut again = Engine::with_bundled_fonts();
    again.open(&bytes, "saved.pptx").unwrap();
    assert_eq!((pictures(&again, 1), pictures(&again, 2)), src_pics);
    // One undo removes both slides and the copied media; redo brings them back.
    let r = run(&mut e, r#"{"cmd":"undo"}"#);
    assert!(!r.can_undo);
    assert_eq!(e.presentation().unwrap(), &before);
    run(&mut e, r#"{"cmd":"redo"}"#);
    assert_eq!(e.presentation().unwrap().slides.len(), n + 2);
    assert_eq!(pictures(&e, 1), src_pics.0);
    // Indices are inserted in the order given; unknown ones are skipped.
    let r = run(&mut e, r#"{"cmd":"reuseSlides","indices":[2,99,0]}"#);
    assert_eq!(r.slide, 2);
    let p = e.presentation().unwrap();
    assert_eq!(p.slides.len(), n + 4);
    assert_eq!(text(p, 2), text(&src_pres, 2));
    assert_eq!(text(p, 3), text(&src_pres, 0));
    assert_package_valid(&e.save_pptx().unwrap());
}

#[test]
fn keep_source_formatting_imports_the_design_once() {
    let src = deck("slide-text-bullets.pptx");
    let src_pres = bo_pptx::read_pptx(&src).unwrap();
    let mut e = Engine::with_bundled_fonts();
    e.open(&bare_target(), "bare.pptx").unwrap();
    let (l0, m0, t0) = {
        let p = e.presentation().unwrap();
        (p.layouts.len(), p.masters.len(), p.themes.len())
    };
    reuse_open(&mut e, &src);
    // Two slides of the same source layout bring one layout, master and theme.
    let same_layout: Vec<usize> = (0..src_pres.slides.len())
        .filter(|&i| src_pres.slides[i].layout == src_pres.slides[1].layout)
        .take(2)
        .collect();
    let list = same_layout
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    run(
        &mut e,
        &format!(r#"{{"cmd":"reuseSlides","indices":[{list}],"keepSource":true}}"#),
    );
    {
        let p = e.presentation().unwrap();
        assert_eq!(
            (p.layouts.len(), p.masters.len(), p.themes.len()),
            (l0 + 1, m0 + 1, t0 + 1)
        );
        assert_eq!(p.slides[1].layout, l0);
        assert_eq!(p.layouts[l0].master, m0);
        assert_eq!(p.masters[m0].theme, t0);
        // The imported design is the source's.
        let sl = &src_pres.layouts[src_pres.slides[1].layout];
        assert_eq!(p.layouts[l0].data.name, sl.data.name);
        assert_eq!(
            p.themes[t0].colors,
            src_pres.themes[src_pres.masters[sl.master].theme].colors
        );
    }
    // Reusing with the source design again finds the imported one.
    run(
        &mut e,
        r#"{"cmd":"reuseSlides","indices":[1],"keepSource":true}"#,
    );
    let p = e.presentation().unwrap();
    assert_eq!(
        (p.layouts.len(), p.masters.len(), p.themes.len()),
        (l0 + 1, m0 + 1, t0 + 1)
    );
    let bytes = e.save_pptx().unwrap();
    let saved = assert_package_valid(&bytes);
    assert_eq!(saved.masters.len(), m0 + 1);
    assert_eq!(text(&saved, 1), text(&src_pres, 1));
    let mut f = Engine::with_bundled_fonts();
    f.open(&bytes, "x.pptx").unwrap();
    assert!(f.slide_json(1).unwrap().len() > 2);
    // Each insertion is one undo step, the imported design goes with the first.
    run(&mut e, r#"{"cmd":"undo"}"#);
    run(&mut e, r#"{"cmd":"undo"}"#);
    let p = e.presentation().unwrap();
    assert_eq!(
        (p.layouts.len(), p.masters.len(), p.themes.len()),
        (l0, m0, t0)
    );
    assert!(!run(&mut e, r#"{"cmd":"state"}"#).can_undo);
}

#[test]
fn destination_design_is_used_without_keep_source() {
    // Same deck family: with the option off the target's layout of the same name is used,
    // even though the target's layout content differs.
    let src = deck("slide-text-bullets.pptx");
    let mut target = bo_pptx::read_pptx(&deck("slide-shapes-presets.pptx")).unwrap();
    for l in &mut target.layouts {
        l.data.shapes.clear();
    }
    let l0 = target.layouts.len();
    let mut e = Engine::with_bundled_fonts();
    e.open(&bo_pptx::write_pptx(&target).unwrap(), "t.pptx")
        .unwrap();
    reuse_open(&mut e, &src);
    run(&mut e, r#"{"cmd":"reuseSlides","indices":[1]}"#);
    assert_eq!(e.presentation().unwrap().layouts.len(), l0);
    run(&mut e, r#"{"cmd":"undo"}"#);
    // With it on, the differing design is imported.
    run(
        &mut e,
        r#"{"cmd":"reuseSlides","indices":[1],"keepSource":true}"#,
    );
    assert_eq!(e.presentation().unwrap().layouts.len(), l0 + 1);
}

#[test]
fn reuse_needs_a_readable_source() {
    let mut e = open("slide-text-bullets.pptx");
    let n = e.presentation().unwrap().slides.len();
    let r = try_run(&mut e, r#"{"cmd":"reuseSlides","indices":[0]}"#);
    assert_eq!(r.error.as_deref(), Some("noReuseSource"));
    // Not a presentation.
    let r = reuse_open(&mut e, b"not a deck");
    assert_eq!(r["error"], "reuseRead");
    let r = try_run(&mut e, r#"{"cmd":"reuseSlides","indices":[0]}"#);
    assert_eq!(r.error.as_deref(), Some("noReuseSource"));
    // No valid index: refused, nothing changes.
    reuse_open(&mut e, &deck("slide-images-crop.pptx"));
    let r = try_run(&mut e, r#"{"cmd":"reuseSlides","indices":[42]}"#);
    assert_eq!(r.error.as_deref(), Some("noSlide"));
    assert_eq!(e.presentation().unwrap().slides.len(), n);
    assert!(!r.can_undo);
    // Closed: the source is forgotten.
    e.slide_command_json(r#"{"cmd":"reuseClose"}"#).unwrap();
    let r = try_run(&mut e, r#"{"cmd":"reuseSlides","indices":[0]}"#);
    assert_eq!(r.error.as_deref(), Some("noReuseSource"));
}
