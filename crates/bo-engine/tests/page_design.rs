//! Design tab over the JSON bridge: page colour, page borders, text and picture
//! watermarks, themes and style sets — drawn on every page, one undo step each,
//! saved and reopened (ECMA-376 Part 1, §17.2.1, §17.6.10; Part 4, §19.1.2.23).
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use bo_render::Command;
use serde_json::Value;

fn cmd(e: &mut Engine, json: &str) -> Value {
    let r: Value = serde_json::from_str(&e.command_json(json).unwrap()).unwrap();
    assert!(r.get("error").is_none(), "{json}: {:?}", r.get("error"));
    r
}

fn reopen(e: &Engine) -> Engine {
    let bytes = e.save_docx().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&bytes, "x.docx").unwrap();
    f
}

/// A new document of about three pages.
fn engine() -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("a.docx").unwrap();
    for i in 0..60 {
        cmd(
            &mut e,
            &format!(
                r#"{{"cmd":"insertText","text":"Đoạn {i} của văn bản thử nghiệm thiết kế trang."}}"#
            ),
        );
        cmd(&mut e, r#"{"cmd":"splitParagraph"}"#);
    }
    e
}

fn page_count(e: &Engine) -> usize {
    e.layout().unwrap().pages.len()
}

#[test]
fn page_colour_is_painted_first_on_every_page_and_undone() {
    let mut e = engine();
    assert!(page_count(&e) >= 2);
    let r = cmd(&mut e, r#"{"cmd":"setPageColor","color":"FDF6E3"}"#);
    assert_eq!(r["design"]["pageColor"], "FDF6E3");
    for i in 0..page_count(&e) {
        let p = e.page(i).unwrap();
        assert!(
            matches!(&p.commands[0], Command::FillRect { w, h, .. } if *w == p.width && *h == p.height),
            "page {i}"
        );
    }
    let saved = reopen(&e);
    assert_eq!(
        saved.document().unwrap().design.background,
        bo_model::Color::from_hex("FDF6E3")
    );
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    let r = cmd(&mut e, r#"{"cmd":"design"}"#);
    assert!(r["design"]["pageColor"].is_null());
    assert!(
        !matches!(e.page(0).unwrap().commands.first(), Some(Command::FillRect { w, .. }) if *w > 500.0)
    );
}

#[test]
fn page_borders_and_art_round_trip() {
    let mut e = engine();
    let r = cmd(
        &mut e,
        r#"{"cmd":"setPageBorders","borders":{"style":"diamondsGray","size":10,"color":"1F6FB2","space":24,"offsetFromPage":true},"scope":"document"}"#,
    );
    let b = &r["design"]["borders"];
    assert_eq!(b["style"], "diamondsGray");
    assert_eq!(b["art"], true);
    assert_eq!(b["sides"], serde_json::json!([true, true, true, true]));
    // Every page carries the four motif edges.
    for p in &e.layout().unwrap().pages {
        assert_eq!(p.decor.borders.len(), 4);
    }
    let f = reopen(&e);
    let pb = f.document().unwrap().sections[0]
        .page_borders
        .clone()
        .unwrap();
    assert_eq!(pb.top.unwrap().line.style, "diamondsGray");
    assert!(pb.offset_from_page);
    cmd(&mut e, r#"{"cmd":"setPageBorders"}"#);
    assert_eq!(e.layout().unwrap().pages[0].decor.borders.len(), 0);
}

#[test]
fn text_watermark_on_every_page_saved_and_removed() {
    let mut e = engine();
    let r = cmd(
        &mut e,
        r#"{"cmd":"setWatermark","watermark":{"text":"BẢN NHÁP","color":"C00000"}}"#,
    );
    let wm = &r["design"]["watermark"];
    assert_eq!(wm["text"], "BẢN NHÁP");
    assert_eq!(wm["semitransparent"], true);
    assert_eq!(wm["diagonal"], true);
    for i in 0..page_count(&e) {
        let p = e.page(i).unwrap();
        assert!(
            p.commands.iter().any(
                |c| matches!(c, Command::PushOpacity { alpha } if (*alpha - 0.5).abs() < 1e-3)
            ),
            "page {i}"
        );
    }
    // Diagonal and automatic size: the rotated box fits the text area.
    let lay = e.layout().unwrap();
    let sec = &e.document().unwrap().sections[0].page;
    let cw = (sec.width.0 - sec.margin_left.0 - sec.margin_right.0) as f32 / 20.0;
    let (bw, bh) = lay.pages[0]
        .items
        .iter()
        .find_map(|i| match i {
            bo_layout::PageItem::Shape { w, h, alpha, .. } if *alpha < 1.0 => Some((*w, *h)),
            _ => None,
        })
        .unwrap();
    assert!(
        (bw + bh) / std::f32::consts::SQRT_2 <= cw + 0.5,
        "{bw} {bh} {cw}"
    );
    let f = reopen(&e);
    let mut f = f;
    let r = cmd(&mut f, r#"{"cmd":"design"}"#);
    assert_eq!(r["design"]["watermark"]["text"], "BẢN NHÁP");
    assert_eq!(r["design"]["watermark"]["color"], "C00000");
    cmd(&mut e, r#"{"cmd":"setWatermark"}"#);
    let r = cmd(&mut e, r#"{"cmd":"design"}"#);
    assert!(r["design"]["watermark"].is_null());
    assert!(!e
        .page(0)
        .unwrap()
        .commands
        .iter()
        .any(|c| matches!(c, Command::PushOpacity { .. })));
}

#[test]
fn picture_watermark_is_washed_out() {
    let mut e = engine();
    // 2×2 PNG.
    let png = bo_raster::rgba_to_png(bo_raster::codecs::Rgba {
        width: 2,
        height: 2,
        pixels: vec![200u8; 16],
    })
    .unwrap();
    let b64 = base64(&png);
    let r = cmd(
        &mut e,
        &format!(r#"{{"cmd":"setWatermark","watermark":{{"picture":"{b64}","scale":5000}}}}"#),
    );
    assert_eq!(r["design"]["watermark"]["picture"], true);
    assert_eq!(r["design"]["watermark"]["washout"], true);
    let p = e.page(0).unwrap();
    let i = p
        .commands
        .iter()
        .position(|c| matches!(c, Command::Image { .. }))
        .unwrap();
    assert!(
        matches!(p.commands[i - 1], Command::PushOpacity { alpha } if (alpha - 0.3).abs() < 1e-3)
    );
    // 2 px at 96 dpi = 1.5 pt, scaled 500 % (the scale is capped) = 7.5 pt.
    assert!(matches!(p.commands[i], Command::Image { w, .. } if (w - 7.5).abs() < 0.01));
    if let Ok(p) = std::env::var("BO_WRITE_DESIGN_PIC") {
        std::fs::write(p, e.save_docx().unwrap()).unwrap();
    }
    let f = reopen(&e);
    assert!(f
        .document()
        .unwrap()
        .media
        .keys()
        .any(|k| k.starts_with("/word/media/watermark")));
}

#[test]
fn theme_and_style_set_from_the_gallery() {
    let mut e = engine();
    let r = cmd(&mut e, r#"{"cmd":"design"}"#);
    assert_eq!(r["design"]["themes"].as_array().unwrap().len(), 4);
    assert_eq!(r["design"]["styleSets"].as_array().unwrap().len(), 4);
    assert!(r["design"]["artStyles"]
        .as_array()
        .unwrap()
        .iter()
        .any(|s| s == "basicBlackDots"));
    let r = cmd(&mut e, r#"{"cmd":"applyTheme","id":"sunset"}"#);
    assert_eq!(r["design"]["theme"], "sunset");
    let r = cmd(&mut e, r#"{"cmd":"applyStyleSet","id":"compact"}"#);
    assert_eq!(r["design"]["styleSet"], "compact");
    let f = reopen(&e);
    let d = f.document().unwrap();
    assert_eq!(d.theme_fonts.major_latin, "Arial");
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    let r = cmd(&mut e, r#"{"cmd":"design"}"#);
    assert!(r["design"]["theme"].is_null());
}

/// Everything at once (BO_WRITE_DESIGN=<path> keeps the file for OpenXmlValidator:
/// `bo-cli validate <path> --openxml require`).
#[test]
fn all_design_features_save_and_reopen() {
    let mut e = engine();
    cmd(&mut e, r#"{"cmd":"setPageColor","color":"E7EEF5"}"#);
    cmd(
        &mut e,
        r#"{"cmd":"setPageBorders","borders":{"style":"double","size":6,"space":20,"shadow":true,"display":"notFirstPage","behind":true}}"#,
    );
    cmd(
        &mut e,
        r#"{"cmd":"setWatermark","watermark":{"text":"MẬT","bold":true}}"#,
    );
    cmd(&mut e, r#"{"cmd":"applyTheme","id":"ocean"}"#);
    cmd(&mut e, r#"{"cmd":"applyStyleSet","id":"balanced"}"#);
    let bytes = e.save_docx().unwrap();
    if let Ok(p) = std::env::var("BO_WRITE_DESIGN") {
        std::fs::write(p, &bytes).unwrap();
    }
    let mut f = Engine::with_bundled_fonts();
    f.open(&bytes, "x.docx").unwrap();
    let r = cmd(&mut f, r#"{"cmd":"design"}"#);
    let d = &r["design"];
    assert_eq!(d["pageColor"], "E7EEF5");
    assert_eq!(d["borders"]["style"], "double");
    assert_eq!(d["borders"]["display"], "notFirstPage");
    assert_eq!(d["borders"]["behind"], true);
    assert_eq!(d["watermark"]["text"], "MẬT");
    assert_eq!(f.document().unwrap().theme_fonts.minor_latin, "Calibri");
    // Saving the reopened document unchanged keeps it.
    let again = f.save_docx().unwrap();
    let mut g = Engine::with_bundled_fonts();
    g.open(&again, "y.docx").unwrap();
    assert_eq!(
        g.document().unwrap().sections[0].page_borders,
        f.document().unwrap().sections[0].page_borders
    );
}

/// The corpus watermarks are read and drawn; changing the section rewrites its
/// headers, whose verbatim picture watermark keeps resolving its picture
/// (BO_WRITE_DESIGN_EDITED=<path> keeps the file for OpenXmlValidator).
#[test]
fn corpus_watermarks_survive_a_section_edit() {
    let src = include_bytes!("../../../tests/corpus/doc/page-02-watermarks.docx");
    let mut e = Engine::with_bundled_fonts();
    e.open(src, "page-02-watermarks.docx").unwrap();
    let r = cmd(&mut e, r#"{"cmd":"design"}"#);
    assert_eq!(r["design"]["watermark"]["text"], "BẢN NHÁP");
    // Page 1 (titlePg) shows the washed-out picture, later pages the text.
    let p0 = e.page(0).unwrap();
    assert!(p0
        .commands
        .iter()
        .any(|c| matches!(c, Command::Image { .. })));
    let p1 = e.page(1).unwrap();
    assert!(p1
        .commands
        .iter()
        .any(|c| matches!(c, Command::PushOpacity { alpha } if (*alpha - 0.5).abs() < 1e-3)));
    cmd(
        &mut e,
        r#"{"cmd":"setPageBorders","borders":{"style":"triangles","size":8,"space":10}}"#,
    );
    let bytes = e.save_docx().unwrap();
    if let Ok(p) = std::env::var("BO_WRITE_DESIGN_EDITED") {
        std::fs::write(p, &bytes).unwrap();
    }
    let mut f = Engine::with_bundled_fonts();
    f.open(&bytes, "x.docx").unwrap();
    let d = f.document().unwrap();
    let first = d.sections[0].headers.first.as_ref().unwrap();
    let pic = first
        .iter()
        .filter_map(bo_model::Block::as_paragraph)
        .flat_map(|p| &p.runs)
        .flat_map(|r| &r.content)
        .find_map(|c| match c {
            bo_model::RunContent::Image(i) => i.shape.as_ref()?.art.as_ref()?.picture.clone(),
            _ => None,
        })
        .unwrap();
    assert!(d.media.contains_key(&pic), "{pic}");
    assert!(f
        .page(0)
        .unwrap()
        .commands
        .iter()
        .any(|c| matches!(c, Command::Image { .. })));
}

fn base64(b: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for c in b.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= c.len() {
                s.push(T[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

/// DOC-PAGE-001: a run coloured from the theme (§17.3.2.6 `w:themeColor`, with a tint)
/// follows the theme when another one is applied, back on undo; the attributes are
/// saved; a colour chosen afterwards is a plain colour.
#[test]
fn theme_colours_of_runs_follow_the_theme() {
    use bo_model::{Block, Document, ParaProps, Paragraph, Run, RunProps, Section, ThemeColorRef};
    let accent = |tint: Option<u8>| RunProps {
        color: bo_model::Color::from_hex("4472C4"),
        theme_color: Some(ThemeColorRef {
            name: "accent1".into(),
            tint,
            shade: None,
        }),
        ..RunProps::default()
    };
    let doc = Document {
        sections: vec![Section::new(vec![Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            vec![
                Run::text("Xanh", accent(None)),
                Run::text(" nhạt", accent(Some(0x80))),
            ],
        ))])],
        ..Document::default()
    };
    let mut e = Engine::with_bundled_fonts();
    e.open(&bo_docx::write_docx(&doc).unwrap(), "t.docx")
        .unwrap();
    // Colour of the fragment showing `word`.
    let of = |e: &Engine, word: &str| -> String {
        e.layout().unwrap().pages[0]
            .lines
            .iter()
            .flat_map(|l| l.fragments.iter())
            .find(|f| f.text.contains(word))
            .map(|f| f.color.to_hex())
            .unwrap_or_default()
    };
    let colours = |e: &Engine| [of(e, "Xanh"), of(e, "nhạt")];
    // No theme part: `w:val` is used.
    assert_eq!(colours(&e), ["4472C4", "4472C4"]);
    cmd(&mut e, r#"{"cmd":"applyTheme","id":"ocean"}"#);
    let ocean = colours(&e);
    assert_eq!(ocean[0], "1F6FB2");
    // Tint 0x80: about halfway to white.
    assert_ne!(ocean[1], "1F6FB2");
    let light = bo_model::Color::from_hex(&ocean[1]).unwrap();
    assert!(
        light.r > 0x1F && light.g > 0x6F && light.b > 0xB2,
        "{light:?}"
    );
    cmd(&mut e, r#"{"cmd":"applyTheme","id":"bamboo"}"#);
    assert_eq!(colours(&e)[0], "3C7D3A");
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(colours(&e), ocean);
    // Saved with the theme part and the attributes; reopened in the same colours.
    let bytes = e.save_docx().unwrap();
    let f = {
        let mut f = Engine::with_bundled_fonts();
        f.open(&bytes, "x.docx").unwrap();
        f
    };
    assert_eq!(colours(&f), ocean);
    let r = &f.document().unwrap().sections[0].blocks[0];
    let Block::Paragraph(p) = r else {
        panic!("{r:?}")
    };
    assert_eq!(
        p.runs[1]
            .props
            .theme_color
            .as_ref()
            .map(|t| (t.name.as_str(), t.tint)),
        Some(("accent1", Some(0x80)))
    );
    // A colour picked for the first word replaces its theme colour.
    cmd(
        &mut e,
        r#"{"cmd":"setSelection","selection":{"anchor":{"p":0,"o":0},"focus":{"p":0,"o":4}}}"#,
    );
    cmd(
        &mut e,
        r#"{"cmd":"formatText","format":{"kind":"color","hex":"FF0000"}}"#,
    );
    cmd(&mut e, r#"{"cmd":"applyTheme","id":"sunset"}"#);
    assert_eq!(of(&e, "Xanh"), "FF0000");
}
