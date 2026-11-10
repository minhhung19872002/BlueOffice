//! BlueSlide hyperlinks and actions (SLIDE-ACTION-001): reading the corpus decks,
//! setting / editing / removing actions through the JSON commands, saving and
//! reopening, the security policy, followed links and PDF link annotations.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::slide_edit::action::{LinkOut, Safety, Trigger};
use bo_engine::slide_edit::present::LinkAction;
use bo_engine::slide_edit::SlideCommand;
use bo_engine::Engine;
use bo_opc::{Limits, Package};
use bo_render::Command;
use serde_json::Value;
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

/// Run a JSON command; the result as JSON.
fn try_run(e: &mut Engine, json: &str) -> Value {
    serde_json::from_str(&e.slide_command_json(json).unwrap()).unwrap()
}

fn run(e: &mut Engine, json: &str) -> Value {
    let r = try_run(e, json);
    assert!(r.get("error").is_none(), "{json}: {}", r["error"]);
    r
}

fn links(e: &mut Engine, slide: usize) -> Vec<LinkOut> {
    let cmd: SlideCommand =
        serde_json::from_str(&format!(r#"{{"cmd":"show","slide":{slide}}}"#)).unwrap();
    e.slide_command(cmd).unwrap().show.unwrap().links
}

fn reopen(e: &Engine) -> Engine {
    let bytes = e.save_pptx().unwrap();
    let mut back = Engine::with_bundled_fonts();
    back.open(&bytes, "back.pptx").unwrap();
    back
}

fn part(bytes: &[u8], name: &str) -> Vec<u8> {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    pkg.part(name)
        .unwrap_or_else(|| panic!("no part {name}"))
        .data
        .clone()
}

fn part_text(bytes: &[u8], name: &str) -> String {
    String::from_utf8(part(bytes, name)).unwrap()
}

const SLIDE1: &str = "/ppt/slides/slide1.xml";
const RELS1: &str = "/ppt/slides/_rels/slide1.xml.rels";

fn url(u: &str) -> LinkAction {
    LinkAction::Url { url: u.into() }
}

// source: ECMA-376 Part 1, §21.1.2.3.5 hlinkClick (rPr and cNvPr), §21.1.2.3.6 hlinkHover,
// §20.1.2.2.32 snd — values as written by `bo-cli sample-slides` (slide_edits::action_decks).
#[test]
fn linked_text_shapes_and_sounds_are_read() {
    let mut e = open("slide-action-links.pptx");
    let all = links(&mut e, 0);
    let text: Vec<&LinkOut> = all.iter().filter(|l| l.text).collect();
    assert_eq!(text.len(), 3, "{all:#?}");
    assert_eq!(text[0].action, LinkAction::Slide { slide: 2 });
    assert_eq!(text[0].tooltip.as_deref(), Some("Đi tới trang 3"));
    assert_eq!(text[0].safety, Safety::Internal);
    let third = e.presentation().unwrap().slides[2].info.part_name.clone();
    assert_eq!(text[0].key, third);
    assert_eq!(text[1].action, url("https://example.vn/blueoffice"));
    assert_eq!(text[1].safety, Safety::External);
    assert_eq!(
        text[2].action,
        url("mailto:lienhe@example.vn?subject=BlueSlide")
    );
    assert_eq!(text[2].safety, Safety::External);
    // The three ranges sit on one line, left to right, inside the text box.
    assert!(text[0].rect[0] < text[1].rect[0] && text[1].rect[0] < text[2].rect[0]);
    assert!(text.iter().all(|l| l.rect[2] > 5.0 && l.rect[3] > 5.0));
    assert!(text.iter().all(|l| l.on == Trigger::Click && l.history));

    let shapes: Vec<&LinkOut> = all.iter().filter(|l| !l.text).collect();
    assert_eq!(shapes.len(), 4, "{shapes:#?}");
    assert_eq!(
        shapes[0].action,
        LinkAction::CustomShow { id: 0, ret: true }
    );
    assert_eq!(
        shapes[1].action,
        LinkAction::File {
            path: "bao-cao.docx".into()
        }
    );
    assert_eq!(shapes[1].safety, Safety::Blocked);
    // The last shape: a click jump and a hover action with a sound and a highlight.
    assert_eq!(shapes[2].shape_id, shapes[3].shape_id);
    assert_eq!(shapes[2].action, LinkAction::LastViewed);
    assert_eq!(shapes[2].on, Trigger::Click);
    let hover = shapes[3];
    assert_eq!(hover.on, Trigger::Hover);
    assert_eq!(hover.action, LinkAction::None);
    assert!(hover.highlight);
    let sound = hover.sound.as_ref().unwrap();
    assert_eq!(sound.name, "chuông.wav");
    let wav = e.media(sound.part.as_deref().unwrap()).unwrap();
    assert_eq!(&wav[..4], b"RIFF");
    // Slides without actions have no hot areas.
    assert_eq!(links(&mut e, 1).len(), 0);
}

#[test]
fn saving_keeps_every_action_as_it_was() {
    for name in [
        "slide-action-links.pptx",
        "slide-action-buttons.pptx",
        "slide-action-unsafe.pptx",
    ] {
        let original = deck(name);
        let mut e = open(name);
        let before = links(&mut e, 0);
        let saved = e.save_pptx().unwrap();
        // Byte for byte: the slide and its relationships.
        assert_eq!(part(&saved, SLIDE1), part(&original, SLIDE1), "{name}");
        assert_eq!(part(&saved, RELS1), part(&original, RELS1), "{name}");
        let mut back = Engine::with_bundled_fonts();
        back.open(&saved, name).unwrap();
        assert_eq!(links(&mut back, 0), before, "{name}");
    }
}

// source: the policy of docs/security.md — programs, macros, files, other decks,
// unknown actions and non-web URLs are reported, kept in the file and never run.
#[test]
fn unsafe_and_unknown_actions_are_blocked_and_cannot_be_created() {
    let mut e = open("slide-action-unsafe.pptx");
    let all = links(&mut e, 0);
    assert_eq!(all.len(), 6, "{all:#?}");
    assert!(all.iter().all(|l| l.safety == Safety::Blocked), "{all:#?}");
    assert_eq!(
        all[0].action,
        LinkAction::Program {
            path: "file:///C:/Windows/System32/calc.exe".into()
        }
    );
    assert_eq!(
        all[1].action,
        LinkAction::Macro {
            name: "TuDongChay".into()
        }
    );
    assert_eq!(
        all[2].action,
        LinkAction::File {
            path: "bao-cao.xlsx".into()
        }
    );
    assert_eq!(
        all[3].action,
        LinkAction::Presentation {
            path: "bai-khac.pptx".into(),
            slide: Some(2)
        }
    );
    assert_eq!(
        all[4].action,
        LinkAction::Unknown {
            action: "ppaction://frobnicate?x=1".into()
        }
    );
    assert_eq!(all[4].on, Trigger::Hover);
    assert_eq!(all[5].action, url("file:///C:/Windows/System32/cmd.exe"));
    // No PDF link for any of them.
    assert_eq!(e.slide_export_links(0).len(), 0);

    // The editor never writes a program, a macro or an unknown action.
    let before = e.save_pptx().unwrap();
    run(
        &mut e,
        &format!(r#"{{"cmd":"select","ids":[{}]}}"#, all[2].shape_id),
    );
    for target in [
        r#"{"kind":"program","path":"file:///C:/Windows/System32/calc.exe"}"#,
        r#"{"kind":"macro","name":"TuDongChay"}"#,
        r#"{"kind":"unknown","action":"ppaction://frobnicate"}"#,
    ] {
        let r = try_run(
            &mut e,
            &format!(r#"{{"cmd":"actionSet","spec":{{"target":{target}}}}}"#),
        );
        assert_eq!(r["error"], "unsafeAction", "{target}");
    }
    assert_eq!(e.save_pptx().unwrap(), before);
    // A control character in a URL is refused too.
    let r = try_run(
        &mut e,
        r#"{"cmd":"actionSet","spec":{"target":{"kind":"url","url":"https://a\u0000b"}}}"#,
    );
    assert_eq!(r["error"], "badLink");
}

#[test]
fn action_buttons_come_with_their_jumps() {
    let mut e = open("slide-action-buttons.pptx");
    let all = links(&mut e, 0);
    let actions: Vec<&LinkAction> = all.iter().map(|l| &l.action).collect();
    assert_eq!(
        actions,
        [
            &LinkAction::First,
            &LinkAction::Prev,
            &LinkAction::Next,
            &LinkAction::Last,
            &LinkAction::First,
            &LinkAction::LastViewed,
            &LinkAction::None,
            &url("https://example.vn/huong-dan"),
        ]
    );
    assert!(all[..7].iter().all(|l| l.highlight));
    // The sound button plays its WAV; the information button stops it.
    assert_eq!(all[6].sound.as_ref().unwrap().name, "bấm.wav");
    assert!(all[7].sound.as_ref().unwrap().stop);
    // Every slide has the six jump buttons.
    for slide in 1..4 {
        assert_eq!(links(&mut e, slide).len(), 6, "slide {slide}");
    }
    // A new button through the command: selected, with its action, one undo step.
    let mut e = open("slide-text-bullets.pptx");
    let r = run(
        &mut e,
        r#"{"cmd":"actionButton","preset":"actionButtonEnd","rect":{"x":10,"y":10,"w":40,"h":40}}"#,
    );
    assert_eq!(r["action"]["scope"], "shape");
    assert_eq!(r["action"]["click"]["target"]["kind"], "last");
    assert_eq!(links(&mut e, 0).len(), 1);
    run(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(links(&mut e, 0).len(), 0);
    let r = try_run(&mut e, r#"{"cmd":"actionButton","preset":"rect"}"#);
    assert_eq!(r["error"], "unknownPreset");
}

#[test]
fn set_edit_and_remove_a_shape_link() {
    let mut e = open("slide-text-bullets.pptx");
    let r = try_run(
        &mut e,
        r#"{"cmd":"actionSet","spec":{"target":{"kind":"next"}}}"#,
    );
    assert_eq!(r["error"], "noSelection");
    run(
        &mut e,
        r#"{"cmd":"insertShape","preset":"rect","rect":{"x":40,"y":40,"w":100,"h":60}}"#,
    );
    // A URL with a tooltip.
    let r = run(
        &mut e,
        r#"{"cmd":"actionSet","spec":{"target":{"kind":"url","url":" https://example.vn/a "},"tooltip":"Mở"}}"#,
    );
    let info = &r["action"];
    assert_eq!(info["scope"], "shape");
    assert_eq!(info["click"]["target"]["url"], "https://example.vn/a");
    assert_eq!(info["click"]["tooltip"], "Mở");
    assert_eq!(info["click"]["safety"], "external");
    assert!(info["hover"].is_null());
    assert!(info["slides"].as_array().unwrap().len() >= 3);
    let bytes = e.save_pptx().unwrap();
    assert!(
        part_text(&bytes, RELS1).contains(r#"Target="https://example.vn/a" TargetMode="External""#)
    );

    // Edit: now a jump to slide 2 — the URL relationship goes, a slide one comes.
    run(
        &mut e,
        r#"{"cmd":"actionSet","spec":{"target":{"kind":"slide","slide":1},"sound":{"kind":"keep"}}}"#,
    );
    let bytes = e.save_pptx().unwrap();
    let rels = part_text(&bytes, RELS1);
    assert!(!rels.contains("example.vn"), "{rels}");
    assert!(
        rels.contains(r#"relationships/slide" Target="slide2.xml""#),
        "{rels}"
    );
    let xml = part_text(&bytes, SLIDE1);
    assert!(xml.contains(r#"action="ppaction://hlinksldjump""#), "{xml}");
    // The tooltip was not given this time: it is gone.
    assert!(!xml.contains("tooltip="), "{xml}");
    let mut back = reopen(&e);
    let l = links(&mut back, 0);
    assert_eq!(l.len(), 1);
    assert_eq!(l[0].action, LinkAction::Slide { slide: 1 });
    assert_eq!(l[0].rect, [40.0, 40.0, 100.0, 60.0]);

    // A hover action next to it (cNvPr: hlinkClick, then hlinkHover).
    run(
        &mut e,
        r#"{"cmd":"actionSet","on":"hover","spec":{"target":{"kind":"end"},"highlight":true}}"#,
    );
    let xml = part_text(&e.save_pptx().unwrap(), SLIDE1);
    let (c, h) = (
        xml.find("<a:hlinkClick").unwrap(),
        xml.find("<a:hlinkHover").unwrap(),
    );
    assert!(c < h, "{xml}");
    assert!(xml.contains(r#"action="ppaction://hlinkshowjump?jump=endshow" highlightClick="1""#));

    // Remove both: no element, no relationship; undo brings them back.
    let r = run(
        &mut e,
        r#"{"cmd":"actionSet","spec":{"target":{"kind":"none"},"sound":{"kind":"none"}}}"#,
    );
    assert!(r["action"]["click"].is_null());
    run(
        &mut e,
        r#"{"cmd":"actionSet","on":"hover","spec":{"target":{"kind":"none"},"sound":{"kind":"none"}}}"#,
    );
    let bytes = e.save_pptx().unwrap();
    assert!(!part_text(&bytes, SLIDE1).contains("hlink"));
    assert!(!part_text(&bytes, RELS1).contains("slide2.xml"));
    assert_eq!(links(&mut e, 0).len(), 0);
    run(&mut e, r#"{"cmd":"undo"}"#);
    run(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(links(&mut e, 0).len(), 2);
    assert!(part_text(&e.save_pptx().unwrap(), RELS1).contains("slide2.xml"));
}

#[test]
fn text_ranges_take_click_and_hover_links() {
    let mut e = open("slide-text-bullets.pptx");
    run(
        &mut e,
        r#"{"cmd":"insertTextBox","text":"Một hai ba","rect":{"x":60,"y":330,"w":400,"h":40}}"#,
    );
    run(&mut e, r#"{"cmd":"textSelect","start":4,"end":7}"#);
    let r = run(
        &mut e,
        r#"{"cmd":"actionSet","spec":{"target":{"kind":"last"}}}"#,
    );
    assert_eq!(r["action"]["scope"], "text");
    assert_eq!(r["action"]["text"], "hai");
    assert_eq!(r["action"]["click"]["target"]["kind"], "last");
    // The selection is kept: a hover action on the same range (§21.1.2.3.7 hlinkMouseOver).
    run(
        &mut e,
        r#"{"cmd":"actionSet","on":"hover","spec":{"target":{"kind":"none"},"highlight":true}}"#,
    );
    let xml = part_text(&e.save_pptx().unwrap(), SLIDE1);
    assert!(
        xml.contains(concat!(
            r#"<a:hlinkClick r:id="" action="ppaction://hlinkshowjump?jump=lastslide"/>"#,
            r#"<a:hlinkMouseOver r:id="" action="ppaction://noaction" highlightClick="1"/>"#,
            r#"</a:rPr><a:t>hai</a:t>"#
        )),
        "{xml}"
    );
    let l = links(&mut e, 0);
    assert_eq!(l.len(), 2);
    assert!(l.iter().all(|x| x.text));
    assert_eq!((l[0].on, l[1].on), (Trigger::Click, Trigger::Hover));
    // Narrower than the whole box: only the word is hot.
    assert!(l[0].rect[2] < 100.0, "{:?}", l[0].rect);

    // The shape itself can be linked while its text is selected (`scope`).
    run(
        &mut e,
        r#"{"cmd":"actionSet","scope":"shape","spec":{"target":{"kind":"first"}}}"#,
    );
    assert_eq!(links(&mut e, 0).len(), 3);

    // Removing the text link joins the runs again (the show ended text editing).
    let id = links(&mut e, 0)[0].shape_id;
    run(&mut e, &format!(r#"{{"cmd":"select","ids":[{id}]}}"#));
    run(&mut e, r#"{"cmd":"textSelect","start":4,"end":7}"#);
    run(
        &mut e,
        r#"{"cmd":"actionSet","spec":{"target":{"kind":"none"},"sound":{"kind":"none"}}}"#,
    );
    run(
        &mut e,
        r#"{"cmd":"actionSet","on":"hover","spec":{"target":{"kind":"none"},"sound":{"kind":"none"}}}"#,
    );
    let xml = part_text(&e.save_pptx().unwrap(), SLIDE1);
    assert!(xml.contains("<a:t>Một hai ba</a:t>"), "{xml}");
    assert!(!xml.contains("hlinkMouseOver"));
}

#[test]
fn sounds_are_embedded_once_and_validated() {
    use base64::Engine as _;
    let b64 = |b: &[u8]| base64::engine::general_purpose::STANDARD.encode(b);
    let mut wav = b"RIFF\x24\x00\x00\x00WAVEfmt ".to_vec();
    wav.extend([0u8; 32]);
    let mut e = open("slide-text-bullets.pptx");
    run(&mut e, r#"{"cmd":"insertShape","preset":"rect"}"#);
    let set = |e: &mut Engine, on: &str, data: &str| {
        try_run(
            e,
            &format!(
                r#"{{"cmd":"actionSet","on":"{on}","spec":{{"target":{{"kind":"none"}},"sound":{{"kind":"file","name":"a.wav","data":"{data}"}}}}}}"#
            ),
        )
    };
    assert_eq!(
        set(&mut e, "click", &b64(b"not a wav file at all"))["error"],
        "badSound"
    );
    assert_eq!(set(&mut e, "click", "%%%")["error"], "badSound");
    assert!(set(&mut e, "click", &b64(&wav)).get("error").is_none());
    assert!(set(&mut e, "hover", &b64(&wav)).get("error").is_none());
    let bytes = e.save_pptx().unwrap();
    let pkg = Package::read(&bytes, &Limits::default()).unwrap();
    let wavs = pkg
        .parts()
        .iter()
        .filter(|p| p.name.ends_with(".wav"))
        .count();
    assert_eq!(wavs, 1, "the same sound is one part");
    let xml = part_text(&bytes, SLIDE1);
    assert_eq!(xml.matches("<a:snd ").count(), 2, "{xml}");
    // "Stop the sound" replaces the sound (endSnd).
    run(
        &mut e,
        r#"{"cmd":"actionSet","spec":{"target":{"kind":"none"},"sound":{"kind":"stop"}}}"#,
    );
    let l = links(&mut e, 0);
    assert!(l[0].sound.as_ref().unwrap().stop);
}

#[test]
fn deleting_a_slide_removes_the_links_to_it() {
    let mut e = open("slide-action-links.pptx");
    assert!(links(&mut e, 0)
        .iter()
        .any(|l| l.action == LinkAction::Slide { slide: 2 }));
    run(&mut e, r#"{"cmd":"deleteSlide","index":2}"#);
    let l = links(&mut e, 0);
    assert_eq!(l.iter().filter(|l| l.text).count(), 2, "{l:#?}");
    let bytes = e.save_pptx().unwrap();
    let rels = part_text(&bytes, RELS1);
    assert!(!rels.contains("relationships/slide\""), "{rels}");
    assert!(!part_text(&bytes, SLIDE1).contains("hlinksldjump"));
    // The text stays, unlinked.
    assert!(part_text(&bytes, SLIDE1).contains("Xem trang 3"));
    // One undo step brings the slide and the link back.
    run(&mut e, r#"{"cmd":"undo"}"#);
    assert!(links(&mut e, 0)
        .iter()
        .any(|l| l.action == LinkAction::Slide { slide: 2 }));
}

/// Colours of the glyph runs of the show layers of slide 0 whose text contains `word`.
fn colours(e: &mut Engine, word: &str) -> Vec<(u8, u8, u8)> {
    let cmd: SlideCommand = serde_json::from_str(r#"{"cmd":"show","slide":0}"#).unwrap();
    let show = e.slide_command(cmd).unwrap().show.unwrap();
    let mut out = Vec::new();
    for layer in &show.layers {
        for c in &layer.page.commands {
            if let Command::Glyphs { text, color, .. } = c {
                if text.contains(word) {
                    out.push((color.0, color.1, color.2));
                }
            }
        }
    }
    out
}

// source: ECMA-376 Part 1, §20.1.6.2 clrScheme — the corpus theme has hlink 0563C1 and
// folHlink 954F72 (bo-cli slide_samples); linked text is drawn in them.
#[test]
fn linked_text_uses_the_theme_link_colours_and_the_followed_state() {
    let mut e = open("slide-action-links.pptx");
    assert_eq!(colours(&mut e, "Xem"), [(0x05, 0x63, 0xC1)]);
    assert_eq!(colours(&mut e, "Bluestar"), [(0x05, 0x63, 0xC1)]);
    let key = links(&mut e, 0)
        .into_iter()
        .find(|l| l.text && l.action == LinkAction::Slide { slide: 2 })
        .unwrap()
        .key;
    run(
        &mut e,
        &format!(r#"{{"cmd":"actionFollowed","key":"{key}"}}"#),
    );
    assert_eq!(colours(&mut e, "Xem"), [(0x95, 0x4F, 0x72)]);
    // Other links keep the hyperlink colour.
    assert_eq!(colours(&mut e, "Bluestar"), [(0x05, 0x63, 0xC1)]);
    // A new show starts with nothing followed; nothing of this is saved or undoable.
    run(&mut e, r#"{"cmd":"actionResetFollowed"}"#);
    assert_eq!(colours(&mut e, "Xem"), [(0x05, 0x63, 0xC1)]);
    assert_eq!(
        part(&e.save_pptx().unwrap(), SLIDE1),
        part(&deck("slide-action-links.pptx"), SLIDE1)
    );
}

// source: ISO 32000-1 §12.5.6.5 link annotations — one per allowed URL and per jump
// whose slide is known; nothing for files, custom shows and "last viewed".
#[test]
fn pdf_export_keeps_urls_and_slide_jumps() {
    let e = open("slide-action-links.pptx");
    for pdf in [
        e.export_pdf().unwrap(),
        e.export_pdf_with(&bo_pdf::ExportChoices::default())
            .unwrap(),
    ] {
        let s = String::from_utf8_lossy(&pdf);
        assert_eq!(s.matches("/Subtype /Link").count(), 3, "links");
        assert!(s.contains("/URI (https://example.vn/blueoffice)"));
        assert!(s.contains("/URI (mailto:lienhe@example.vn?subject=BlueSlide)"));
        assert_eq!(s.matches("/S /GoTo").count(), 1);
        assert!(!s.contains("bao-cao.docx"));
    }
    let e = open("slide-action-buttons.pptx");
    let s = e.export_pdf().unwrap();
    let s = String::from_utf8_lossy(&s);
    // Slide 1: first, next, last, home + the web link (no previous slide, no "last viewed");
    // slides 2–3: five jumps each; slide 4: no next.
    assert_eq!(s.matches("/S /GoTo").count(), 4 + 5 + 5 + 4);
    assert_eq!(s.matches("/S /URI").count(), 1);
    let e = open("slide-action-unsafe.pptx");
    let s = e.export_pdf().unwrap();
    assert!(!String::from_utf8_lossy(&s).contains("/Subtype /Link"));
}
