//! Slide masters, layouts, themes and slide size (SLIDE-MASTER-001) through the
//! JSON bridge: master view, layout / placeholder edits, several masters,
//! themes and variants, slide size scale vs fit; undo and lossless saving.
//!
//! With `BO_MASTER_OUT=<dir>` the written files are kept there (for
//! OpenXmlValidator: `bo-cli validate <dir> --openxml require`).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use base64::Engine as _;
use bo_engine::Engine;
use bo_model::slide::{Background, ColorKind, Fill, Presentation, ShapeNode, Xfrm};
use bo_pptx::build::{
    build_pptx, DeckSpec, LayoutKind, ParaSpec, Placeholder, ShapeSpec, SlideSpec,
};
use serde_json::{json, Value};
use std::path::PathBuf;

fn keep(name: &str, bytes: &[u8]) {
    if let Ok(dir) = std::env::var("BO_MASTER_OUT") {
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(PathBuf::from(dir).join(name), bytes).unwrap();
    }
}

/// 16:9 deck: a title slide, then a "title and content" slide with a free
/// rectangle at (1 219 200, 685 800) EMU, 2 438 400 × 1 371 600 EMU.
fn deck_bytes() -> Vec<u8> {
    let title = |p: Placeholder, text: &str| ShapeSpec {
        name: "Title".into(),
        x: 838_200,
        y: 365_125,
        cx: 10_515_600,
        cy: 1_325_563,
        placeholder: Some(p),
        text: vec![ParaSpec::plain(text)],
        ..ShapeSpec::default()
    };
    build_pptx(&DeckSpec {
        width: 12_192_000,
        height: 6_858_000,
        title: None,
        slides: vec![
            SlideSpec {
                layout: LayoutKind::Title,
                shapes: vec![title(Placeholder::CenterTitle, "Mở đầu")],
                ..SlideSpec::default()
            },
            SlideSpec {
                layout: LayoutKind::TitleContent,
                shapes: vec![
                    title(Placeholder::Title, "Nội dung"),
                    ShapeSpec {
                        name: "Box".into(),
                        x: 1_219_200,
                        y: 685_800,
                        cx: 2_438_400,
                        cy: 1_371_600,
                        ..ShapeSpec::default()
                    },
                ],
                ..SlideSpec::default()
            },
        ],
    })
    .unwrap()
}

fn open() -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.open(&deck_bytes(), "deck.pptx").unwrap();
    e
}

fn run_json(e: &mut Engine, v: &Value) -> Value {
    let s = e.slide_command_json(&v.to_string()).unwrap();
    let r: Value = serde_json::from_str(&s).unwrap();
    assert!(r.get("error").is_none(), "{v}: {:?}", r.get("error"));
    r
}

fn error_of(e: &mut Engine, v: &Value) -> Option<String> {
    let s = e.slide_command_json(&v.to_string()).unwrap();
    let r: Value = serde_json::from_str(&s).unwrap();
    r.get("error").and_then(Value::as_str).map(str::to_owned)
}

fn pres(e: &Engine) -> &Presentation {
    e.presentation().unwrap()
}

fn xfrm_of(n: &ShapeNode) -> Option<Xfrm> {
    match n {
        ShapeNode::Shape(s) => s.props.xfrm.clone(),
        _ => None,
    }
}

fn ph_kind(n: &ShapeNode) -> Option<String> {
    match n {
        ShapeNode::Shape(s) => {
            s.nv.placeholder
                .as_ref()
                .map(|p| p.kind.clone().unwrap_or_else(|| "obj".into()))
        }
        _ => None,
    }
}

fn part(bytes: &[u8], name: &str) -> Option<Vec<u8>> {
    let pkg = bo_opc::Package::read(bytes, &bo_opc::Limits::default()).unwrap();
    pkg.part(name).map(|p| p.data.clone())
}

fn part_names(bytes: &[u8]) -> Vec<String> {
    let pkg = bo_opc::Package::read(bytes, &bo_opc::Limits::default()).unwrap();
    pkg.parts().iter().map(|p| p.name.clone()).collect()
}

#[test]
fn opening_and_closing_the_master_view_changes_nothing() {
    let mut e = open();
    let a = e.save_pptx().unwrap();
    let r = run_json(&mut e, &json!({"cmd":"masterView","on":true}));
    // One master and its three layouts (build: title, title and content, blank).
    let items = r["masterView"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 4);
    assert_eq!(items[0]["kind"], "master");
    assert_eq!(items[1]["kind"], "layout");
    // The view opens on the current slide's layout (slide 1 uses the title layout).
    assert_eq!(r["slide"], 1);
    assert_eq!(r["info"]["pageCount"], 4);
    // Saving while the view is open writes the real deck.
    let during = e.save_pptx().unwrap();
    assert_eq!(during, a);
    let r = run_json(&mut e, &json!({"cmd":"masterView","on":false}));
    assert!(r.get("masterView").is_none());
    assert_eq!(r["info"]["pageCount"], 2);
    assert_eq!(r["canUndo"], false);
    let b = e.save_pptx().unwrap();
    assert_eq!(a, b, "byte-stable after an idle master view session");
    // Write → read → write keeps masters, layouts and themes byte for byte.
    let again = bo_pptx::write_pptx(&bo_pptx::read_pptx(&b).unwrap()).unwrap();
    for name in part_names(&b) {
        if name.contains("slideMaster") || name.contains("slideLayout") || name.contains("theme") {
            assert_eq!(part(&b, &name), part(&again, &name), "{name}");
        }
    }
    keep("idle.pptx", &b);
}

#[test]
fn new_layout_placeholder_is_inherited_by_slides() {
    let mut e = open();
    run_json(&mut e, &json!({"cmd":"masterView","on":true}));
    let r = run_json(&mut e, &json!({"cmd":"insertLayout","name":"Ảnh lớn"}));
    let cur = r["slide"].as_u64().unwrap() as usize;
    assert_eq!(r["masterView"]["items"][cur]["name"], "Ảnh lớn");
    assert_eq!(r["masterView"]["items"].as_array().unwrap().len(), 5);
    // Inserted after the title layout (view slide 1): it is view slide 2.
    assert_eq!(cur, 2);
    let r = run_json(
        &mut e,
        &json!({"cmd":"insertPlaceholder","kind":"picture","rect":{"x":100,"y":50,"w":200,"h":100}}),
    );
    // Box of the new placeholder: centre (100 + 200/2, 50 + 100/2) = (200, 100) pt.
    let b = &r["selection"][0]["rect"];
    assert_eq!(
        (b["cx"].as_f64(), b["cy"].as_f64()),
        (Some(200.0), Some(100.0))
    );
    assert_eq!(r["selection"][0]["placeholder"], "pic");
    // Copied into the layout at once (live).
    let layout = pres(&e)
        .layouts
        .iter()
        .position(|l| l.data.name.as_deref() == Some("Ảnh lớn"))
        .unwrap();
    let pic = pres(&e).layouts[layout]
        .data
        .shapes
        .iter()
        .find(|n| ph_kind(n).as_deref() == Some("pic"))
        .cloned()
        .unwrap();
    // 100 pt = 1 270 000 EMU, 50 pt = 635 000, 200 pt = 2 540 000, 100 pt = 1 270 000.
    let x = xfrm_of(&pic).unwrap();
    assert_eq!(x.off, Some((1_270_000, 635_000)));
    assert_eq!(x.ext, Some((2_540_000, 1_270_000)));
    let r = run_json(&mut e, &json!({"cmd":"masterView","on":false}));
    assert_eq!(r["canUndo"], true);
    let layouts = &r["design"]["masters"][0]["layouts"];
    let idx = layouts
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["name"] == "Ảnh lớn")
        .unwrap()["index"]
        .as_u64()
        .unwrap();
    assert_eq!(idx as usize, layout);
    // A new slide on that layout: its picture placeholder has no box of its
    // own and shows the layout's.
    run_json(&mut e, &json!({"cmd":"addSlide","after":1,"layout":idx}));
    let r = run_json(&mut e, &json!({"cmd":"selectAll"}));
    let pic = r["selection"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["placeholder"] == "pic")
        .unwrap();
    assert_eq!(pic["rect"]["cx"], 200.0);
    assert_eq!(pic["rect"]["cy"], 100.0);
    assert_eq!(pic["rect"]["w"], 200.0);
    assert_eq!(pic["rect"]["h"], 100.0);
    let s = &pres(&e).slides[2];
    let own = s
        .data
        .shapes
        .iter()
        .find(|n| ph_kind(n).as_deref() == Some("pic"))
        .unwrap();
    assert!(xfrm_of(own).is_none());
    let bytes = e.save_pptx().unwrap();
    keep("new-layout.pptx", &bytes);
    let back = bo_pptx::read_pptx(&bytes).unwrap();
    assert_eq!(back.layouts.len(), 4);
    assert_eq!(
        back.layouts[back.slides[2].layout].data.name.as_deref(),
        Some("Ảnh lớn")
    );
    // Undo the new slide, then the whole master session.
    run_json(&mut e, &json!({"cmd":"undo"}));
    run_json(&mut e, &json!({"cmd":"undo"}));
    assert_eq!(pres(&e).layouts.len(), 3);
    assert_eq!(pres(&e).slides.len(), 2);
    run_json(&mut e, &json!({"cmd":"redo"}));
    assert_eq!(pres(&e).layouts.len(), 4);
}

#[test]
fn master_edits_reach_layouts_live_and_undo_in_the_view() {
    let mut e = open();
    run_json(&mut e, &json!({"cmd":"masterView","on":true}));
    // A new layout's title has no box: it follows the master title
    // (build master: x = 12 192 000 / 24 = 508 000 EMU = 40 pt,
    // w = 12 192 000 − 2 · 508 000 = 11 176 000 EMU = 880 pt → centre 480 pt).
    run_json(&mut e, &json!({"cmd":"insertLayout","name":"Riêng"}));
    let master_title = pres(&e).masters[0]
        .data
        .shapes
        .iter()
        .find(|n| ph_kind(n).as_deref() == Some("title"))
        .and_then(|n| n.nv().map(|nv| nv.id))
        .unwrap();
    run_json(&mut e, &json!({"cmd":"goTo","slide":0}));
    run_json(&mut e, &json!({"cmd":"select","ids":[master_title]}));
    run_json(
        &mut e,
        &json!({"cmd":"transform","transform":{"kind":"move","dx":10,"dy":0},"snap":false}),
    );
    // The master itself changed at once: 508 000 + 10 · 12 700 = 635 000 EMU.
    let off = pres(&e).masters[0]
        .data
        .shapes
        .iter()
        .find(|n| ph_kind(n).as_deref() == Some("title"))
        .and_then(xfrm_of)
        .and_then(|x| x.off)
        .unwrap();
    assert_eq!(off.0, 635_000);
    // The new layout (view slide 2) shows its title at 490 pt.
    run_json(&mut e, &json!({"cmd":"goTo","slide":2}));
    let r = run_json(&mut e, &json!({"cmd":"selectAll"}));
    let t = r["selection"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["placeholder"] == "title")
        .unwrap();
    assert_eq!(t["rect"]["cx"], 490.0);
    // Undo inside the view puts the master back.
    run_json(&mut e, &json!({"cmd":"undo"}));
    let off = pres(&e).masters[0]
        .data
        .shapes
        .iter()
        .find(|n| ph_kind(n).as_deref() == Some("title"))
        .and_then(xfrm_of)
        .and_then(|x| x.off)
        .unwrap();
    assert_eq!(off.0, 508_000);
    // Slide-list commands do not apply in the view.
    assert_eq!(
        error_of(&mut e, &json!({"cmd":"addSlide"})).as_deref(),
        Some("masterView")
    );
    // Layout used by slides cannot be deleted; an unused one can.
    run_json(&mut e, &json!({"cmd":"goTo","slide":1}));
    assert_eq!(
        error_of(&mut e, &json!({"cmd":"deleteMasterItem"})).as_deref(),
        Some("layoutInUse")
    );
    run_json(&mut e, &json!({"cmd":"goTo","slide":2}));
    let r = run_json(&mut e, &json!({"cmd":"renameMasterItem","name":"Đổi tên"}));
    assert_eq!(r["masterView"]["items"][2]["name"], "Đổi tên");
    let r = run_json(&mut e, &json!({"cmd":"deleteMasterItem"}));
    assert_eq!(r["masterView"]["items"].as_array().unwrap().len(), 4);
    // The only master cannot be deleted.
    run_json(&mut e, &json!({"cmd":"goTo","slide":0}));
    assert_eq!(
        error_of(&mut e, &json!({"cmd":"deleteMasterItem"})).as_deref(),
        Some("lastMaster")
    );
    // Hide background graphics on the blank layout, background on the master.
    run_json(&mut e, &json!({"cmd":"goTo","slide":3}));
    run_json(&mut e, &json!({"cmd":"hideBackgroundGraphics","hide":true}));
    run_json(&mut e, &json!({"cmd":"goTo","slide":0}));
    run_json(&mut e, &json!({"cmd":"setBackground","color":"203040"}));
    run_json(&mut e, &json!({"cmd":"preserveMaster","on":true}));
    run_json(&mut e, &json!({"cmd":"masterView","on":false}));
    let p = pres(&e);
    assert_eq!(p.layouts.len(), 3);
    assert_eq!(p.layouts[2].show_master_sp, Some(false));
    match &p.masters[0].data.background {
        Some(Background::Props {
            fill: Fill::Solid(c),
            ..
        }) => {
            assert_eq!(c.kind, ColorKind::Srgb(0x0020_3040));
        }
        other => panic!("{other:?}"),
    }
    let bytes = e.save_pptx().unwrap();
    keep("master-edits.pptx", &bytes);
    let xml =
        String::from_utf8(part(&bytes, "/ppt/slideMasters/slideMaster1.xml").unwrap()).unwrap();
    assert!(xml.contains(r#"preserve="1""#));
    assert!(xml.contains(r#"<a:srgbClr val="203040"/>"#));
    let lay =
        String::from_utf8(part(&bytes, "/ppt/slideLayouts/slideLayout3.xml").unwrap()).unwrap();
    assert!(lay.contains(r#"showMasterSp="0""#));
}

#[test]
fn master_title_style_changes_slide_titles() {
    let mut e = open();
    let title_id = |e: &Engine| {
        pres(e).slides[1]
            .data
            .shapes
            .iter()
            .find(|n| ph_kind(n).as_deref() == Some("title"))
            .and_then(|n| n.nv().map(|nv| nv.id))
            .unwrap()
    };
    let id = title_id(&e);
    run_json(&mut e, &json!({"cmd":"goTo","slide":1}));
    let r = run_json(&mut e, &json!({"cmd":"select","ids":[id]}));
    // build master titleStyle: sz = 4000 → 40 pt.
    assert_eq!(r["format"]["size"], 40.0);
    assert_eq!(r["format"]["bold"], false);
    // Through the master view, on a layout: the style belongs to the master.
    run_json(&mut e, &json!({"cmd":"masterView","on":true}));
    run_json(
        &mut e,
        &json!({"cmd":"masterTextStyle","style":"title","size":54,"bold":true,"color":"8B0000"}),
    );
    run_json(&mut e, &json!({"cmd":"masterView","on":false}));
    let r = run_json(&mut e, &json!({"cmd":"select","ids":[id]}));
    assert_eq!(r["format"]["size"], 54.0);
    assert_eq!(r["format"]["bold"], true);
    assert_eq!(r["format"]["color"], "8B0000");
    // Body style, one level, in the normal view.
    run_json(
        &mut e,
        &json!({"cmd":"masterTextStyle","style":"body","level":2,"size":30}),
    );
    let ts = &pres(&e).masters[0].text_styles;
    assert_eq!(
        ts.body.as_ref().unwrap().levels[1]
            .as_ref()
            .unwrap()
            .def_rpr
            .as_ref()
            .unwrap()
            .sz,
        Some(3000)
    );
    // Level 1 kept its 28 pt.
    assert_eq!(
        ts.body.as_ref().unwrap().levels[0]
            .as_ref()
            .unwrap()
            .def_rpr
            .as_ref()
            .unwrap()
            .sz,
        Some(2800)
    );
    run_json(&mut e, &json!({"cmd":"undo"}));
    run_json(&mut e, &json!({"cmd":"undo"}));
    let r = run_json(&mut e, &json!({"cmd":"select","ids":[id]}));
    assert_eq!(r["format"]["size"], 40.0);
}

#[test]
fn several_masters_round_trip_with_unique_ids() {
    let mut e = open();
    run_json(&mut e, &json!({"cmd":"masterView","on":true}));
    let r = run_json(&mut e, &json!({"cmd":"addMaster","name":"Thứ hai"}));
    let items = r["masterView"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 8);
    assert_eq!(items[4]["kind"], "master");
    assert_eq!(items[4]["name"], "Thứ hai");
    assert_eq!(r["slide"], 4);
    // Give the second master its own theme.
    run_json(&mut e, &json!({"cmd":"applyTheme","builtin":"ember"}));
    let r = run_json(&mut e, &json!({"cmd":"masterView","on":false}));
    let masters = r["design"]["masters"].as_array().unwrap();
    assert_eq!(masters.len(), 2);
    assert_eq!(masters[1]["theme"], "BlueOffice Ember");
    assert_eq!(masters[0]["theme"], "BlueSlide");
    // Base slide 2 on the second master's "title and content" layout.
    let lay = masters[1]["layouts"][1]["index"].as_u64().unwrap();
    run_json(
        &mut e,
        &json!({"cmd":"applyLayout","layout":lay,"slides":[1]}),
    );
    assert_eq!(pres(&e).layouts[pres(&e).slides[1].layout].master, 1);
    let bytes = e.save_pptx().unwrap();
    keep("two-masters.pptx", &bytes);
    let back = bo_pptx::read_pptx(&bytes).unwrap();
    assert_eq!(back.masters.len(), 2);
    assert_eq!(back.layouts.len(), 6);
    assert_eq!(back.themes.len(), 2);
    assert_eq!(back.layouts[back.slides[1].layout].master, 1);
    assert_eq!(
        back.themes[back.masters[1].theme].name.as_deref(),
        Some("BlueOffice Ember")
    );
    // §19.2.1.27 / §19.3.1.41: master and layout ids are unique in the package.
    let mut ids: Vec<u32> = back
        .masters
        .iter()
        .flat_map(|m| std::iter::once(m.master_id).chain(m.layouts.iter().map(|l| l.id)))
        .collect();
    let n = ids.len();
    assert_eq!(n, 8);
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), n);
    assert!(ids.iter().all(|&i| i >= 2_147_483_648));
    // Second save of the read-back deck keeps masters / layouts / themes byte for byte.
    let again = bo_pptx::write_pptx(&back).unwrap();
    for name in part_names(&bytes) {
        if name.contains("slideMaster") || name.contains("slideLayout") || name.contains("theme") {
            assert_eq!(part(&bytes, &name), part(&again, &name), "{name}");
        }
    }
    // Undo the layout change and the master session.
    run_json(&mut e, &json!({"cmd":"undo"}));
    run_json(&mut e, &json!({"cmd":"undo"}));
    assert_eq!(pres(&e).masters.len(), 1);
    assert_eq!(pres(&e).themes.len(), 1);
}

#[test]
fn slide_size_scale_versus_fit() {
    let mut e = open();
    let box_xfrm = |e: &Engine| {
        pres(e).slides[1]
            .data
            .shapes
            .iter()
            .find(|n| n.nv().is_some_and(|nv| nv.name == "Box"))
            .and_then(xfrm_of)
            .unwrap()
    };
    let master_title = |e: &Engine| {
        pres(e).masters[0]
            .data
            .shapes
            .iter()
            .find(|n| ph_kind(n).as_deref() == Some("title"))
            .and_then(xfrm_of)
            .unwrap()
    };
    // 16:9 (12 192 000 × 6 858 000) → 4:3 (9 144 000 × 6 858 000):
    // sx = 0.75, sy = 1.
    // Fit: k = 0.75, dx = 0, dy = (6 858 000 − 6 858 000 · 0.75) / 2 = 857 250.
    //   Box (1 219 200, 685 800) 2 438 400 × 1 371 600 →
    //   (914 400, 514 350 + 857 250 = 1 371 600) 1 828 800 × 1 028 700.
    let r = run_json(
        &mut e,
        &json!({"cmd":"setSlideSize","cx":9_144_000,"cy":6_858_000,"kind":"screen4x3","mode":"fit"}),
    );
    assert_eq!(r["design"]["cx"], 9_144_000);
    assert_eq!(r["design"]["sizeKind"], "screen4x3");
    assert_eq!(r["info"]["pages"][0]["width"], 720.0);
    let x = box_xfrm(&e);
    assert_eq!(x.off, Some((914_400, 1_371_600)));
    assert_eq!(x.ext, Some((1_828_800, 1_028_700)));
    // Master title (508 000, 342 900) 11 176 000 × 1 143 000 →
    // (381 000, 257 175 + 857 250 = 1 114 425) 8 382 000 × 857 250.
    let t = master_title(&e);
    assert_eq!(t.off, Some((381_000, 1_114_425)));
    assert_eq!(t.ext, Some((8_382_000, 857_250)));
    run_json(&mut e, &json!({"cmd":"undo"}));
    assert_eq!(pres(&e).slide_size.cx, 12_192_000);
    assert_eq!(box_xfrm(&e).off, Some((1_219_200, 685_800)));
    // Scale: k = 1, dx = (9 144 000 − 12 192 000) / 2 = −1 524 000, dy = 0.
    //   Box → (−304 800, 685 800), same size.
    run_json(
        &mut e,
        &json!({"cmd":"setSlideSize","cx":9_144_000,"cy":6_858_000,"kind":"screen4x3","mode":"scale"}),
    );
    let x = box_xfrm(&e);
    assert_eq!(x.off, Some((-304_800, 685_800)));
    assert_eq!(x.ext, Some((2_438_400, 1_371_600)));
    // Back to 16:9 with "fit" from 4:3: k = min(4/3, 1) = 1, dx = 1 524 000 →
    //   the box returns to (1 219 200, 685 800).
    run_json(
        &mut e,
        &json!({"cmd":"setSlideSize","cx":12_192_000,"cy":6_858_000,"mode":"fit"}),
    );
    let x = box_xfrm(&e);
    assert_eq!(x.off, Some((1_219_200, 685_800)));
    assert_eq!(
        error_of(
            &mut e,
            &json!({"cmd":"setSlideSize","cx":100,"cy":6_858_000,"mode":"fit"})
        )
        .as_deref(),
        Some("badSize")
    );
    let bytes = e.save_pptx().unwrap();
    keep("slide-size.pptx", &bytes);
    let xml = String::from_utf8(part(&bytes, "/ppt/presentation.xml").unwrap()).unwrap();
    assert!(
        xml.contains(r#"<p:sldSz cx="12192000" cy="6858000"/>"#),
        "{xml}"
    );
}

/// Theme part of `bytes` with `extra` inserted before `</a:theme>`.
#[test]
fn slide_size_scales_font_sizes() {
    let mut e = open();
    let title_sz = |e: &Engine| {
        pres(e).masters[0]
            .text_styles
            .title
            .as_ref()
            .and_then(|t| t.levels[0].as_ref())
            .and_then(|l| l.def_rpr.as_ref())
            .and_then(|r| r.sz)
    };
    let body_sz = |e: &Engine, lvl: usize| {
        pres(e).masters[0]
            .text_styles
            .body
            .as_ref()
            .and_then(|t| t.levels[lvl].as_ref())
            .and_then(|l| l.def_rpr.as_ref())
            .and_then(|r| r.sz)
    };
    // build master: titleStyle 40 pt, bodyStyle level 1 28 pt.
    assert_eq!(title_sz(&e), Some(4000));
    assert_eq!(body_sz(&e, 0), Some(2800));
    // A run with its own size on slide 2's title.
    let title_id = pres(&e).slides[1]
        .data
        .shapes
        .iter()
        .find(|n| ph_kind(n).as_deref() == Some("title"))
        .and_then(|n| n.nv().map(|nv| nv.id))
        .unwrap();
    run_json(&mut e, &json!({"cmd":"goTo","slide":1}));
    run_json(&mut e, &json!({"cmd":"select","ids":[title_id]}));
    run_json(
        &mut e,
        &json!({"cmd":"formatText","format":{"kind":"size","points":36}}),
    );
    // 16:9 → 4:3 fit: k = 9 144 000 / 12 192 000 = 0.75 → 40 → 30 pt,
    // 28 → 21 pt, the run's 36 → 27 pt.
    run_json(
        &mut e,
        &json!({"cmd":"setSlideSize","cx":9_144_000,"cy":6_858_000,"kind":"screen4x3","mode":"fit"}),
    );
    assert_eq!(title_sz(&e), Some(3000));
    assert_eq!(body_sz(&e, 0), Some(2100));
    let r = run_json(&mut e, &json!({"cmd":"select","ids":[title_id]}));
    assert_eq!(r["format"]["size"], 27.0);
    // Undo restores every size.
    run_json(&mut e, &json!({"cmd":"undo"}));
    assert_eq!(title_sz(&e), Some(4000));
    let r = run_json(&mut e, &json!({"cmd":"select","ids":[title_id]}));
    assert_eq!(r["format"]["size"], 36.0);
    // Scale from 16:9 to 16:10 (12 192 000 × 7 620 000): k = max(1, 1.111) = 10/9,
    // 40 pt → 44.44 pt (4444 hundredths).
    run_json(
        &mut e,
        &json!({"cmd":"setSlideSize","cx":12_192_000,"cy":7_620_000,"mode":"scale"}),
    );
    assert_eq!(title_sz(&e), Some(4444));
    let xml = String::from_utf8(
        part(&e.save_pptx().unwrap(), "/ppt/slideMasters/slideMaster1.xml").unwrap(),
    )
    .unwrap();
    assert!(xml.contains(r#"sz="4444""#), "{xml}");
}

#[test]
fn formatting_master_placeholders_changes_the_master_styles() {
    let mut e = open();
    run_json(&mut e, &json!({"cmd":"masterView","on":true}));
    let id_of = |e: &Engine, slide: usize, kind: &str| {
        pres(e).slides[slide]
            .data
            .shapes
            .iter()
            .find(|n| ph_kind(n).as_deref() == Some(kind))
            .and_then(|n| n.nv().map(|nv| nv.id))
            .unwrap()
    };
    // View slide 0 is the master: its title placeholder → p:titleStyle.
    let t = id_of(&e, 0, "title");
    run_json(&mut e, &json!({"cmd":"goTo","slide":0}));
    run_json(&mut e, &json!({"cmd":"select","ids":[t]}));
    run_json(
        &mut e,
        &json!({"cmd":"formatText","format":{"kind":"bold","value":true}}),
    );
    run_json(
        &mut e,
        &json!({"cmd":"formatText","format":{"kind":"color","hex":"1F4E79"}}),
    );
    // The body placeholder: every level of p:bodyStyle gets the size.
    let b = id_of(&e, 0, "body");
    run_json(&mut e, &json!({"cmd":"select","ids":[b]}));
    run_json(
        &mut e,
        &json!({"cmd":"formatText","format":{"kind":"size","points":20}}),
    );
    {
        let ts = &pres(&e).masters[0].text_styles;
        let lvl1 = ts.title.as_ref().unwrap().levels[0].as_ref().unwrap();
        let r = lvl1.def_rpr.as_ref().unwrap();
        assert_eq!(r.b, Some(true));
        assert!(matches!(
            &r.fill,
            Some(Fill::Solid(c)) if matches!(c.kind, ColorKind::Srgb(0x1F_4E_79))
        ));
        // Title size untouched.
        assert_eq!(r.sz, Some(4000));
        let body = ts.body.as_ref().unwrap();
        for l in body.levels.iter().flatten() {
            assert_eq!(l.def_rpr.as_ref().unwrap().sz, Some(2000));
        }
    }
    // A layout's title placeholder: its own a:lstStyle (the master stays).
    let lay_slide = (1..pres(&e).slides.len())
        .find(|&i| {
            pres(&e).slides[i]
                .data
                .shapes
                .iter()
                .any(|n| ph_kind(n).as_deref() == Some("ctrTitle"))
        })
        .unwrap();
    let lt = id_of(&e, lay_slide, "ctrTitle");
    run_json(&mut e, &json!({"cmd":"goTo","slide":lay_slide}));
    run_json(&mut e, &json!({"cmd":"select","ids":[lt]}));
    run_json(
        &mut e,
        &json!({"cmd":"formatText","format":{"kind":"italic","value":true}}),
    );
    run_json(&mut e, &json!({"cmd":"masterView","on":false}));
    let p = pres(&e);
    let lay = &p.layouts[p.slides[0].layout];
    let ls = lay
        .data
        .shapes
        .iter()
        .find(|n| ph_kind(n).as_deref() == Some("ctrTitle"))
        .and_then(|n| match n {
            ShapeNode::Shape(s) => s.text.as_ref().and_then(|t| t.list_style.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        ls.levels[0].as_ref().unwrap().def_rpr.as_ref().unwrap().i,
        Some(true)
    );
    assert_eq!(
        p.masters[0].text_styles.title.as_ref().unwrap().levels[0]
            .as_ref()
            .unwrap()
            .def_rpr
            .as_ref()
            .unwrap()
            .i,
        None
    );
    // The slides inherit: slide 2's title (title layout of master 1) is bold,
    // slide 1's centred title (the edited layout) is bold and italic.
    let id = |e: &Engine, s: usize, k: &str| {
        pres(e).slides[s]
            .data
            .shapes
            .iter()
            .find(|n| ph_kind(n).as_deref() == Some(k))
            .and_then(|n| n.nv().map(|nv| nv.id))
            .unwrap()
    };
    let t2 = id(&e, 1, "title");
    run_json(&mut e, &json!({"cmd":"goTo","slide":1}));
    let r = run_json(&mut e, &json!({"cmd":"select","ids":[t2]}));
    assert_eq!(r["format"]["bold"], true);
    assert_eq!(r["format"]["color"], "1F4E79");
    assert_eq!(r["format"]["italic"], false);
    let t1 = id(&e, 0, "ctrTitle");
    run_json(&mut e, &json!({"cmd":"goTo","slide":0}));
    let r = run_json(&mut e, &json!({"cmd":"select","ids":[t1]}));
    assert_eq!(r["format"]["bold"], true);
    assert_eq!(r["format"]["italic"], true);
    // The whole master session is one step.
    run_json(&mut e, &json!({"cmd":"undo"}));
    assert_ne!(
        pres(&e).masters[0].text_styles.title.as_ref().unwrap().levels[0]
            .as_ref()
            .unwrap()
            .def_rpr
            .as_ref()
            .unwrap()
            .b,
        Some(true)
    );
    let bytes = e.save_pptx().unwrap();
    keep("master-format-styles.pptx", &bytes);
}

fn with_theme_ext(bytes: &[u8], extra: &str) -> Vec<u8> {
    let mut pkg = bo_opc::Package::read(bytes, &bo_opc::Limits::default()).unwrap();
    let xml = String::from_utf8(pkg.part("/ppt/theme/theme1.xml").unwrap().data.clone()).unwrap();
    let xml = xml.replace("</a:theme>", &format!("{extra}</a:theme>"));
    pkg.put_part("/ppt/theme/theme1.xml", xml.into_bytes());
    pkg.write().unwrap()
}

const EXT: &str = r#"<a:extLst><a:ext uri="{B0B0B0B0-1111-2222-3333-444455556666}"><bo:mark xmlns:bo="urn:blueoffice:test" v="1"/></a:ext></a:extLst>"#;

/// A `.thmx` package: the theme part is the main part (§14.2.7).
fn thmx() -> Vec<u8> {
    let theme = format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Thử nghiệm">"#,
            r#"<a:themeElements><a:clrScheme name="Thử"><a:dk1><a:srgbClr val="111111"/></a:dk1><a:lt1><a:srgbClr val="FEFEFE"/></a:lt1>"#,
            r#"<a:dk2><a:srgbClr val="222222"/></a:dk2><a:lt2><a:srgbClr val="EEEEEE"/></a:lt2>"#,
            r#"<a:accent1><a:srgbClr val="123456"/></a:accent1><a:accent2><a:srgbClr val="234567"/></a:accent2>"#,
            r#"<a:accent3><a:srgbClr val="345678"/></a:accent3><a:accent4><a:srgbClr val="456789"/></a:accent4>"#,
            r#"<a:accent5><a:srgbClr val="56789A"/></a:accent5><a:accent6><a:srgbClr val="6789AB"/></a:accent6>"#,
            r#"<a:hlink><a:srgbClr val="0000FF"/></a:hlink><a:folHlink><a:srgbClr val="800080"/></a:folHlink></a:clrScheme>"#,
            r#"<a:fontScheme name="Thử"><a:majorFont><a:latin typeface="Tinos"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont>"#,
            r#"<a:minorFont><a:latin typeface="Carlito"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme>"#,
            r#"<a:fmtScheme name="Thử"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:fillStyleLst>"#,
            r#"<a:lnStyleLst><a:ln w="6350"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="12700"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln><a:ln w="19050"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:ln></a:lnStyleLst>"#,
            r#"<a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst>"#,
            r#"<a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme>"#,
            r#"</a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/>{ext}</a:theme>"#
        ),
        ext = EXT
    );
    let mut pkg = bo_opc::Package::new();
    pkg.put_part("/theme/theme/theme1.xml", theme.into_bytes());
    pkg.set_relationships(
        "/",
        &[bo_opc::Relationship {
            id: "rId1".into(),
            rel_type: bo_opc::REL_OFFICE_DOCUMENT.into(),
            target: "theme/theme/theme1.xml".into(),
            mode: bo_opc::TargetMode::Internal,
        }],
    );
    pkg.set_content_types(&bo_opc::ContentTypes {
        defaults: vec![
            (
                "rels".into(),
                "application/vnd.openxmlformats-package.relationships+xml".into(),
            ),
            ("xml".into(), "application/xml".into()),
        ],
        overrides: vec![(
            "/theme/theme/theme1.xml".into(),
            "application/vnd.openxmlformats-officedocument.theme+xml".into(),
        )],
    });
    pkg.write().unwrap()
}

#[test]
fn themes_and_variants_keep_unknown_theme_parts() {
    let bytes = with_theme_ext(&deck_bytes(), EXT);
    let mut e = Engine::with_bundled_fonts();
    e.open(&bytes, "deck.pptx").unwrap();
    // Colour variant: the scheme changes, the theme's extLst stays.
    let r = run_json(
        &mut e,
        &json!({"cmd":"themeVariant","colors":"bamboo","fonts":"tinos"}),
    );
    assert_eq!(r["design"]["colors"], "BlueOffice Bamboo");
    assert_eq!(r["design"]["fonts"], "BlueOffice Tinos");
    let out = e.save_pptx().unwrap();
    keep("variant.pptx", &out);
    let xml = String::from_utf8(part(&out, "/ppt/theme/theme1.xml").unwrap()).unwrap();
    assert!(xml.contains("urn:blueoffice:test"), "{xml}");
    assert!(
        xml.contains(r#"<a:accent1><a:srgbClr val="2E7D32"/></a:accent1>"#),
        "{xml}"
    );
    assert!(
        xml.contains(r#"<a:majorFont><a:latin typeface="Tinos"/>"#),
        "{xml}"
    );
    assert!(
        xml.contains(r#"<a:minorFont><a:latin typeface="Liberation Sans"/>"#),
        "{xml}"
    );
    // A BlueOffice theme replaces the whole theme.
    let r = run_json(&mut e, &json!({"cmd":"applyTheme","builtin":"harbor"}));
    assert_eq!(r["design"]["theme"], "BlueOffice Harbor");
    let t = &pres(&e).themes[0];
    assert_eq!(t.info.part_name, "/ppt/theme/theme1.xml");
    assert_eq!(t.fonts.major.latin.typeface, "Carlito");
    assert_eq!(
        t.scheme_color("accent1").map(|c| c.kind.clone()),
        Some(ColorKind::Srgb(0x001B_6CA8))
    );
    // A theme file: its unknown parts come along.
    let b64 = base64::engine::general_purpose::STANDARD.encode(thmx());
    let r = run_json(&mut e, &json!({"cmd":"applyTheme","file":b64,"all":true}));
    assert_eq!(r["design"]["theme"], "Thử nghiệm");
    let out = e.save_pptx().unwrap();
    keep("thmx.pptx", &out);
    let xml = String::from_utf8(part(&out, "/ppt/theme/theme1.xml").unwrap()).unwrap();
    assert!(xml.contains("urn:blueoffice:test"));
    assert!(xml.contains(r#"<a:accent1><a:srgbClr val="123456"/></a:accent1>"#));
    // From another deck's first master.
    let other = with_theme_ext(&deck_bytes(), "");
    let b64 = base64::engine::general_purpose::STANDARD.encode(other);
    let r = run_json(&mut e, &json!({"cmd":"applyTheme","file":b64}));
    assert_eq!(r["design"]["theme"], "BlueSlide");
    assert_eq!(
        error_of(&mut e, &json!({"cmd":"applyTheme","builtin":"nope"})).as_deref(),
        Some("noTheme")
    );
    // Each step is one undo.
    run_json(&mut e, &json!({"cmd":"undo"}));
    run_json(&mut e, &json!({"cmd":"undo"}));
    run_json(&mut e, &json!({"cmd":"undo"}));
    assert_eq!(pres(&e).themes[0].colors.name, "BlueOffice Bamboo");
}

#[test]
fn background_and_hidden_graphics_on_slides() {
    let mut e = open();
    run_json(&mut e, &json!({"cmd":"goTo","slide":1}));
    run_json(
        &mut e,
        &json!({"cmd":"setBackground","color":"FFF5E1","all":true}),
    );
    for s in &pres(&e).slides {
        assert!(matches!(
            &s.data.background,
            Some(Background::Props { fill: Fill::Solid(c), .. }) if c.kind == ColorKind::Srgb(0x00FF_F5E1)
        ));
    }
    run_json(&mut e, &json!({"cmd":"hideBackgroundGraphics","hide":true}));
    let bytes = e.save_pptx().unwrap();
    keep("slide-bg.pptx", &bytes);
    let xml = String::from_utf8(part(&bytes, "/ppt/slides/slide2.xml").unwrap()).unwrap();
    assert!(xml.contains(r#"showMasterSp="0""#));
    run_json(&mut e, &json!({"cmd":"setBackground"}));
    assert!(pres(&e).slides[1].data.background.is_none());
    assert!(pres(&e).slides[0].data.background.is_some());
}

#[test]
fn saving_in_the_master_view_clears_modified() {
    let mut e = open();
    run_json(&mut e, &json!({"cmd":"masterView","on":true}));
    let r = run_json(&mut e, &json!({"cmd":"insertLayout","name":"Mới"}));
    assert_eq!(r["modified"], true);
    let bytes = e.save_pptx().unwrap();
    e.mark_saved();
    assert_eq!(bo_pptx::read_pptx(&bytes).unwrap().layouts.len(), 4);
    let r = run_json(&mut e, &json!({"cmd":"state"}));
    assert_eq!(r["modified"], false);
    let r = run_json(&mut e, &json!({"cmd":"masterView","on":false}));
    assert_eq!(r["modified"], false);
    assert_eq!(r["canUndo"], true);
    let r = run_json(&mut e, &json!({"cmd":"undo"}));
    assert_eq!(r["modified"], true);
}
