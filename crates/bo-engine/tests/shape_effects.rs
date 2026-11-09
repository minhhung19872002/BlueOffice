//! Shape effects and WordArt (CORE-SHAPE-002): reflection, soft edges, 3-D rotation, text
//! warp, text outline and fill — commands, display lists, PNG / PDF output and round trip.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::slide_edit::{SlideCommand, SlideEditResult};
use bo_engine::Engine;
use bo_model::slide::{Effect, Fill, ShapeNode};
use bo_render::{Command, Paint, PathSeg};
use std::path::PathBuf;

fn deck(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

fn open() -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.open(&deck("slide-edit-arrange.pptx"), "fx.pptx").unwrap();
    // An empty slide to draw on.
    run(&mut e, r#"{"cmd":"addSlide"}"#);
    let n = e.presentation().unwrap().slides.len();
    run(&mut e, &format!(r#"{{"cmd":"goTo","slide":{}}}"#, n - 1));
    let ids: Vec<u32> = e.presentation().unwrap().slides[n - 1]
        .data
        .shapes
        .iter()
        .filter_map(|s| match s {
            ShapeNode::Shape(s) => Some(s.nv.id),
            _ => None,
        })
        .collect();
    if !ids.is_empty() {
        run(&mut e, &format!(r#"{{"cmd":"select","ids":{ids:?}}}"#));
        run(&mut e, r#"{"cmd":"delete"}"#);
    }
    e
}

fn run(e: &mut Engine, json: &str) -> SlideEditResult {
    let cmd: SlideCommand =
        serde_json::from_str(json).unwrap_or_else(|err| panic!("{json}: {err}"));
    let r = e.slide_command(cmd).unwrap();
    assert!(r.error.is_none(), "{json}: {:?}", r.error);
    r
}

fn current(e: &Engine) -> usize {
    e.presentation().unwrap().slides.len() - 1
}

fn shape(e: &Engine, id: u32) -> bo_model::slide::Shape {
    let p = e.presentation().unwrap();
    p.slides[current(e)]
        .data
        .shapes
        .iter()
        .find_map(|n| match n {
            ShapeNode::Shape(s) if s.nv.id == id => Some((**s).clone()),
            _ => None,
        })
        .unwrap()
}

/// A 200 × 100 pt rectangle at (100, 100).
fn rect(e: &mut Engine) -> u32 {
    let r = run(
        e,
        r#"{"cmd":"insertShape","preset":"rect","rect":{"x":100,"y":100,"w":200,"h":100}}"#,
    );
    r.selection[0].id
}

fn page(e: &mut Engine) -> Vec<Command> {
    let i = current(e);
    e.refresh_slide(i).unwrap().commands.clone()
}

fn pixel(e: &mut Engine, x: u32, y: u32) -> [u8; 4] {
    let i = current(e);
    let page = e.refresh_slide(i).unwrap().clone();
    let pm = bo_raster::render_page(&page, e.fonts(), 1.0).unwrap();
    let k = ((y * pm.width() + x) * 4) as usize;
    let d = pm.data();
    [d[k], d[k + 1], d[k + 2], d[k + 3]]
}

fn bbox(segs: &[PathSeg]) -> [f32; 4] {
    let mut b = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for s in segs {
        let pts: Vec<(f32, f32)> = match *s {
            PathSeg::M(x, y) | PathSeg::L(x, y) => vec![(x, y)],
            PathSeg::Q(a, b2, x, y) => vec![(a, b2), (x, y)],
            PathSeg::C(a, b2, c, d, x, y) => vec![(a, b2), (c, d), (x, y)],
            PathSeg::Z => vec![],
        };
        for (x, y) in pts {
            b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
        }
    }
    b
}

#[test]
fn reflection_mirrors_the_shape_below_it_and_fades() {
    let mut e = open();
    let id = rect(&mut e);
    let before = pixel(&mut e, 200, 212);
    run(
        &mut e,
        r#"{"cmd":"shapeEffects","op":{"kind":"reflection","size":0.5,"distance":4}}"#,
    );
    // §20.1.8.50: written with every attribute of the preset.
    let s = shape(&e, id);
    let fx = s.props.effects.clone().unwrap();
    let Some(Effect::Other(x)) = fx
        .iter()
        .find(|f| matches!(f, Effect::Other(x) if x.name == "a:reflection"))
    else {
        panic!("{fx:?}")
    };
    assert_eq!(x.attr("endPos"), Some("50000"));
    assert_eq!(x.attr("dist"), Some("50800"));
    assert_eq!(x.attr("sy"), Some("-100000"));
    assert_eq!(x.attr("algn"), Some("bl"));
    let cmds = page(&mut e);
    // Mirrored about the bottom edge y = 200, moved 4 pt down: y' = 404 − y.
    assert!(
        cmds.iter().any(|c| matches!(c, Command::PushTransform { m }
            if (m[0] - 1.0).abs() < 1e-4 && (m[3] + 1.0).abs() < 1e-4 && (m[5] - 404.0).abs() < 1e-3)),
        "no mirror transform"
    );
    // Alpha ramp: stA 50 % at the top band (position 1/48), fading to 0 at 50 %:
    // 0.5 · (1 − (1/48)/0.5) = 0.479.
    let alphas: Vec<f32> = cmds
        .iter()
        .filter_map(|c| match c {
            Command::PushOpacity { alpha } => Some(*alpha),
            _ => None,
        })
        .collect();
    assert!(
        alphas.iter().any(|a| (a - 0.479).abs() < 0.003),
        "{alphas:?}"
    );
    // On screen: tinted just below the shape (top of the reflection), clear far below.
    let after = pixel(&mut e, 200, 212);
    assert_eq!(before, [255, 255, 255, 255]);
    assert!(
        after[0] < 240 || after[1] < 240 || after[2] < 240,
        "{after:?}"
    );
    let far = pixel(&mut e, 200, 300);
    assert!(far.iter().take(3).all(|c| *c > 250), "{far:?}");
    // PDF: the bands become constant-opacity groups.
    let pdf = String::from_utf8_lossy(&e.export_pdf().unwrap()).into_owned();
    assert!(pdf.contains("/ca 0.47"), "no reflection opacity in the PDF");
    // Removing it restores the drawing.
    run(
        &mut e,
        r#"{"cmd":"shapeEffects","op":{"kind":"reflection"}}"#,
    );
    assert_eq!(pixel(&mut e, 200, 212), before);
}

#[test]
fn soft_edges_fade_inwards_over_the_radius() {
    let mut e = open();
    let id = rect(&mut e);
    let inside = pixel(&mut e, 200, 150);
    run(
        &mut e,
        r#"{"cmd":"shapeEffects","op":{"kind":"softEdge","radius":8}}"#,
    );
    let s = shape(&e, id);
    let fx = s.props.effects.clone().unwrap();
    assert!(fx
        .iter()
        .any(|f| matches!(f, Effect::Other(x) if x.name == "a:softEdge" && x.attr("rad") == Some("101600"))));
    let cmds = page(&mut e);
    // Eight copies, each clipped to the outline inset by a further 1 pt.
    let clips: Vec<[f32; 4]> = cmds
        .iter()
        .filter_map(|c| match c {
            Command::PushClipPath { segs } => Some(bbox(segs)),
            _ => None,
        })
        .collect();
    assert_eq!(clips.len(), 8, "{clips:?}");
    let close = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-3);
    assert!(
        close(clips[0], [101.0, 101.0, 299.0, 199.0]),
        "{:?}",
        clips[0]
    );
    assert!(
        close(clips[7], [108.0, 108.0, 292.0, 192.0]),
        "{:?}",
        clips[7]
    );
    // Fully opaque deeper than the radius, faded within it, clear on the outline.
    assert_eq!(pixel(&mut e, 200, 150), inside);
    let edge = pixel(&mut e, 200, 100);
    let near = pixel(&mut e, 200, 104);
    let lum = |p: [u8; 4]| u32::from(p[0]) + u32::from(p[1]) + u32::from(p[2]);
    assert!(
        lum(edge) > lum(near) && lum(near) > lum(inside),
        "{edge:?} {near:?} {inside:?}"
    );
    assert!(e.render_png(current(&e), 0.5).unwrap().len() > 100);
}

#[test]
fn rotation_3d_writes_a_scene_and_turns_the_face() {
    let mut e = open();
    let id = rect(&mut e);
    let flat = page(&mut e);
    run(
        &mut e,
        r#"{"cmd":"shapeEffects","op":{"kind":"rotation3d","preset":"isometricLeftDown"}}"#,
    );
    let s = shape(&e, id);
    let scene = s
        .props
        .extra
        .iter()
        .find(|x| x.xml.name == "a:scene3d")
        .unwrap();
    let cam = scene.xml.child("a:camera").unwrap();
    assert_eq!(cam.attr("prst"), Some("isometricLeftDown"));
    assert!(scene.xml.child("a:lightRig").is_some());
    assert_ne!(page(&mut e), flat);
    let bad: SlideCommand = serde_json::from_str(
        r#"{"cmd":"shapeEffects","op":{"kind":"rotation3d","preset":"<x/>"}}"#,
    )
    .unwrap();
    assert!(e.slide_command(bad).unwrap().error.is_some());
    run(
        &mut e,
        r#"{"cmd":"shapeEffects","op":{"kind":"rotation3d"}}"#,
    );
    assert_eq!(page(&mut e), flat);
}

fn text_box(e: &mut Engine, text: &str) -> u32 {
    let r = run(
        e,
        &format!(
            r#"{{"cmd":"insertTextBox","text":{},"rect":{{"x":100,"y":100,"w":300,"h":80}}}}"#,
            serde_json::json!(text)
        ),
    );
    r.selection[0].id
}

fn paths(cmds: &[Command]) -> Vec<(Vec<PathSeg>, Option<Paint>, Option<bo_render::Stroke>)> {
    cmds.iter()
        .filter_map(|c| match c {
            Command::Path { segs, fill, stroke } => {
                Some((segs.clone(), fill.clone(), stroke.clone()))
            }
            _ => None,
        })
        .collect()
}

/// Page boxes of the paths drawn under a transform (shape text is drawn in local coordinates).
fn page_paths(cmds: &[Command]) -> Vec<[f32; 4]> {
    let mut stack: Vec<[f32; 6]> = vec![[1.0, 0.0, 0.0, 1.0, 0.0, 0.0]];
    let mut out = Vec::new();
    for c in cmds {
        match c {
            Command::PushTransform { m } => {
                let t = *stack.last().unwrap();
                let n = [
                    t[0] * m[0] + t[2] * m[1],
                    t[1] * m[0] + t[3] * m[1],
                    t[0] * m[2] + t[2] * m[3],
                    t[1] * m[2] + t[3] * m[3],
                    t[0] * m[4] + t[2] * m[5] + t[4],
                    t[1] * m[4] + t[3] * m[5] + t[5],
                ];
                stack.push(n);
            }
            Command::PopTransform => {
                stack.pop();
            }
            // Text paths only: they are drawn inside the shape's text transform.
            Command::Path { segs, .. } if stack.len() > 1 => {
                let t = *stack.last().unwrap();
                let b = bbox(segs);
                let p = |x: f32, y: f32| (t[0] * x + t[2] * y + t[4], t[1] * x + t[3] * y + t[5]);
                let (a, c2) = (p(b[0], b[1]), p(b[2], b[3]));
                out.push([a.0.min(c2.0), a.1.min(c2.1), a.0.max(c2.0), a.1.max(c2.1)]);
            }
            _ => {}
        }
    }
    out
}

#[test]
fn text_warp_bends_the_glyph_outlines() {
    let mut e = open();
    let id = text_box(&mut e, "WAVE TEXT");
    let flat = page(&mut e);
    assert!(flat.iter().any(|c| matches!(c, Command::Glyphs { .. })));
    run(
        &mut e,
        r#"{"cmd":"shapeEffects","op":{"kind":"textWarp","preset":"textWave1"}}"#,
    );
    let s = shape(&e, id);
    let tb = s.text.as_ref().unwrap();
    let w = tb
        .body_pr
        .extra
        .iter()
        .find(|x| x.xml.name == "a:prstTxWarp")
        .unwrap();
    assert_eq!(w.xml.attr("prst"), Some("textWave1"));
    assert!(w.xml.child("a:avLst").is_some());
    let cmds = page(&mut e);
    // The glyph runs became warped outline paths.
    assert!(!cmds.iter().any(|c| matches!(c, Command::Glyphs { .. })));
    let text = page_paths(&cmds);
    assert_ne!(text.len(), 0);
    // An envelope warp stretches the text over the text box: 7.2 pt insets left and right
    // of the 300 pt wide box at x = 100.
    let x0 = text.iter().map(|b| b[0]).fold(f32::MAX, f32::min);
    let x1 = text.iter().map(|b| b[2]).fold(f32::MIN, f32::max);
    assert!(
        (x0 - 107.2).abs() < 0.5 && (x1 - 392.8).abs() < 0.5,
        "{x0} {x1}"
    );
    // Every warp of the gallery draws; previews come with the command.
    let r = run(
        &mut e,
        r#"{"cmd":"shapeEffects","op":{"kind":"warpPreviews"}}"#,
    );
    let previews = r.warp_previews.unwrap();
    assert_eq!(previews.len(), 40);
    assert!(previews
        .iter()
        .all(|p| p.d.starts_with('M') && p.d.len() > 50));
    // Removing the warp brings the glyph runs back.
    run(&mut e, r#"{"cmd":"shapeEffects","op":{"kind":"textWarp"}}"#);
    assert_eq!(page(&mut e), flat);
}

#[test]
fn text_outline_and_gradient_fill() {
    let mut e = open();
    let id = text_box(&mut e, "Outline");
    run(
        &mut e,
        r#"{"cmd":"shapeEffects","op":{"kind":"textOutline","color":"C00000","width":1.5}}"#,
    );
    run(
        &mut e,
        r#"{"cmd":"shapeEffects","op":{"kind":"textFill","fill":{"type":"gradient","from":"FFC000","to":"0070C0","angle":90}}}"#,
    );
    let s = shape(&e, id);
    let tb = s.text.as_ref().unwrap();
    let bo_model::slide::TextRun::Run { props, .. } = &tb.paragraphs[0].runs[0] else {
        panic!()
    };
    assert_eq!(props.line.as_ref().unwrap().w, Some(19_050));
    assert!(matches!(props.fill, Some(Fill::Gradient(_))));
    let cmds = page(&mut e);
    let p = paths(&cmds);
    assert!(
        p.iter()
            .any(|(_, f, _)| matches!(f, Some(Paint::Linear { stops, .. })
            if stops.len() == 2 && stops[0].color == bo_render::Rgba(0xFF, 0xC0, 0, 255))),
        "no gradient text"
    );
    assert!(p.iter().any(|(_, _, s)| matches!(s, Some(st)
        if st.color == bo_render::Rgba(0xC0, 0, 0, 255) && (st.width - 1.5).abs() < 1e-4)));
    // PDF: a shading for the gradient, the outline stroked.
    let pdf = String::from_utf8_lossy(&e.export_pdf().unwrap()).into_owned();
    assert!(
        pdf.contains("/ShadingType 2") || pdf.contains("/Shading"),
        "no shading"
    );
    // Outline only.
    run(
        &mut e,
        r#"{"cmd":"shapeEffects","op":{"kind":"textFill","fill":{"type":"none"}}}"#,
    );
    let p = paths(&page(&mut e));
    assert!(p.iter().any(|(_, f, s)| f.is_none() && s.is_some()));
    // Back to plain text.
    run(
        &mut e,
        r#"{"cmd":"shapeEffects","op":{"kind":"textFill","fill":{"type":"auto"}}}"#,
    );
    run(
        &mut e,
        r#"{"cmd":"shapeEffects","op":{"kind":"textOutline"}}"#,
    );
    assert!(page(&mut e)
        .iter()
        .any(|c| matches!(c, Command::Glyphs { text, .. } if text == "Outline")));
}

/// Every attribute of a reflection, a soft edge and a warp with adjust values survives a
/// save; a second save is byte-identical (untouched effects are stable).
#[test]
fn effects_round_trip() {
    let mut e = open();
    let id = rect(&mut e);
    let tid = text_box(&mut e, "Arch");
    {
        let i = current(&e);
        let p = e.presentation_mut().unwrap();
        for n in &mut p.slides[i].data.shapes {
            let ShapeNode::Shape(s) = n else { continue };
            if s.nv.id == id {
                let x = bo_drawingml::dom::parse(
                    br#"<a:effectLst xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:reflection blurRad="12700" stA="45000" stPos="1000" endA="2000" endPos="60000" dist="25400" dir="5400000" fadeDir="5400000" sx="90000" sy="-80000" kx="600000" ky="-300000" algn="b" rotWithShape="0"/><a:softEdge rad="63500"/></a:effectLst>"#,
                    "/x",
                )
                .unwrap();
                s.props.effects = Some(bo_drawingml::props::parse_effects(&x));
            }
            if s.nv.id == tid {
                let w = bo_drawingml::dom::parse(
                    br#"<a:prstTxWarp xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" prst="textArchUp"><a:avLst><a:gd name="adj" fmla="val 10800000"/></a:avLst></a:prstTxWarp>"#,
                    "/x",
                )
                .unwrap();
                let tb = s.text.as_mut().unwrap();
                tb.body_pr.extra.push(bo_model::slide::Extra {
                    key: bo_drawingml::dom::key_for(bo_drawingml::text::BODY_PR, "a:prstTxWarp", 0),
                    xml: w,
                });
            }
        }
    }
    let a = e.save_pptx().unwrap();
    let mut e2 = Engine::with_bundled_fonts();
    e2.open(&a, "a.pptx").unwrap();
    let i = current(&e2);
    let s = e2.presentation().unwrap().slides[i]
        .data
        .shapes
        .iter()
        .find_map(|n| match n {
            ShapeNode::Shape(s) if s.nv.id == id => Some((**s).clone()),
            _ => None,
        })
        .unwrap();
    let fx = s.props.effects.unwrap();
    let Effect::Other(r) = &fx[0] else { panic!() };
    assert_eq!(r.name, "a:reflection");
    for (k, v) in [
        ("blurRad", "12700"),
        ("stA", "45000"),
        ("stPos", "1000"),
        ("endA", "2000"),
        ("endPos", "60000"),
        ("dist", "25400"),
        ("fadeDir", "5400000"),
        ("sx", "90000"),
        ("sy", "-80000"),
        ("kx", "600000"),
        ("ky", "-300000"),
        ("algn", "b"),
        ("rotWithShape", "0"),
    ] {
        assert_eq!(r.attr(k), Some(v), "{k}");
    }
    assert!(matches!(&fx[1], Effect::Other(x) if x.attr("rad") == Some("63500")));
    // The deck still renders (warped arch, reflection, soft edge) in every backend.
    assert!(e2.render_png(i, 0.5).unwrap().len() > 100);
    assert!(e2.export_pdf().unwrap().len() > 100);
    let b = e2.save_pptx().unwrap();
    let part = |bytes: &[u8]| {
        let pkg = bo_opc::Package::read(bytes, &bo_opc::Limits::default()).unwrap();
        pkg.parts()
            .iter()
            .filter(|p| p.name.starts_with("/ppt/slides/slide"))
            .map(|p| (p.name.clone(), p.data.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(part(&a), part(&b));
    if let Ok(dir) = std::env::var("BO_FX_OUT") {
        std::fs::write(PathBuf::from(dir).join("shape-effects.pptx"), &b).unwrap();
    }
}

/// A deck built with every effect command (written out with `BO_FX_OUT` for the
/// OpenXmlValidator / visual checks).
#[test]
fn every_command_saves_and_reopens() {
    let mut e = open();
    rect(&mut e);
    for op in [
        r#"{"kind":"reflection","size":0.55,"distance":4}"#,
        r#"{"kind":"softEdge","radius":5}"#,
    ] {
        run(&mut e, &format!(r#"{{"cmd":"shapeEffects","op":{op}}}"#));
    }
    for (k, preset) in ["perspectiveLeft", "isometricLeftDown", "obliqueTopRight"]
        .iter()
        .enumerate()
    {
        run(
            &mut e,
            &format!(
                r#"{{"cmd":"insertShape","preset":"rect","rect":{{"x":{},"y":100,"w":120,"h":80}}}}"#,
                400 + k * 170
            ),
        );
        run(
            &mut e,
            &format!(
                r#"{{"cmd":"shapeEffects","op":{{"kind":"rotation3d","preset":"{preset}"}}}}"#
            ),
        );
    }
    run(
        &mut e,
        r#"{"cmd":"insertTextBox","text":"BlueSlide","rect":{"x":100,"y":300,"w":400,"h":120}}"#,
    );
    for op in [
        r#"{"kind":"textWarp","preset":"textArchUp"}"#,
        r#"{"kind":"textOutline","color":"002060","width":1}"#,
        r#"{"kind":"textFill","fill":{"type":"gradient","from":"FFC000","to":"C00000","angle":90}}"#,
    ] {
        run(&mut e, &format!(r#"{{"cmd":"shapeEffects","op":{op}}}"#));
    }
    let bytes = e.save_pptx().unwrap();
    let mut e2 = Engine::with_bundled_fonts();
    e2.open(&bytes, "b.pptx").unwrap();
    let i = current(&e2);
    let again = e2.refresh_slide(i).unwrap().clone();
    assert_eq!(&again, e.refresh_slide(i).unwrap());
    if let Ok(dir) = std::env::var("BO_FX_OUT") {
        std::fs::write(
            PathBuf::from(&dir).join("shape-effects-commands.pptx"),
            &bytes,
        )
        .unwrap();
        std::fs::write(
            PathBuf::from(&dir).join("shape-effects-commands.png"),
            e2.render_png(i, 1.0).unwrap(),
        )
        .unwrap();
    }
}
