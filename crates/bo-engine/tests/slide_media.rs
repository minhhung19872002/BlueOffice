//! BlueSlide audio / video playback options (SLIDE-MEDIA-001): trim, fades and bookmarks
//! ([MS-PPTX] §2.3.1.18 p14:media), captions ([MS-PPTX] §2.13 tracksInfo + WebVTT Track
//! parts), the media time node (ECMA-376 Part 1, §19.5 p:video / p:cMediaNode) and the start
//! effect in the timing tree. Hand-written fixtures; expected values read off the XML.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::slide_edit::media_playback::{MediaCommand, MediaPlayback};
use bo_engine::slide_edit::{SlideCommand, SlideEditResult};
use bo_engine::Engine;
use bo_model::slide::{XmlElement, XmlNode};
use bo_opc::{Package, Relationship, TargetMode};
use std::path::PathBuf;

fn corpus(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/slide")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

const NS: &str = concat!(
    r#"xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" "#,
    r#"xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" "#,
    r#"xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main""#
);

/// The bookmark-triggered sequence: shape 20 fades in when clip 4 reaches "Mốc 1".
const BMK_SEQ: &str = concat!(
    r#"<p:seq concurrent="1" nextAc="seek"><p:cTn id="20" restart="whenNotActive" fill="hold" evtFilter="cancelBubble" nodeType="interactiveSeq">"#,
    r#"<p:stCondLst><p:cond evt="onMediaBookmark" delay="0"><p:tgtEl><p14:bmkTgt spid="4" bmkName="Mốc 1"/></p:tgtEl></p:cond></p:stCondLst>"#,
    r#"<p:endSync evt="end" delay="0"><p:rtn val="all"/></p:endSync><p:childTnLst>"#,
    r#"<p:par><p:cTn id="21" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst>"#,
    r#"<p:par><p:cTn id="22" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst>"#,
    r#"<p:par><p:cTn id="23" presetID="10" presetClass="entr" presetSubtype="0" fill="hold" nodeType="clickEffect">"#,
    r#"<p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst>"#,
    r#"<p:set><p:cBhvr><p:cTn id="24" dur="1" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst></p:cTn><p:tgtEl><p:spTgt spid="20"/></p:tgtEl><p:attrNameLst><p:attrName>style.visibility</p:attrName></p:attrNameLst></p:cBhvr><p:to><p:strVal val="visible"/></p:to></p:set>"#,
    r#"<p:animEffect transition="in" filter="fade"><p:cBhvr><p:cTn id="25" dur="500"/><p:tgtEl><p:spTgt spid="20"/></p:tgtEl></p:cBhvr></p:animEffect>"#,
    r#"</p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par>"#,
    r#"</p:childTnLst></p:cTn><p:nextCondLst><p:cond evt="onMediaBookmark" delay="0"><p:tgtEl><p14:bmkTgt spid="4" bmkName="Mốc 1"/></p:tgtEl></p:cond></p:nextCondLst></p:seq>"#
);

/// Main sequence: on the first click, play clip 4 (§19.5 cmd `playFrom`).
const MAIN_SEQ: &str = concat!(
    r#"<p:seq concurrent="1" nextAc="seek"><p:cTn id="2" dur="indefinite" nodeType="mainSeq"><p:childTnLst>"#,
    r#"<p:par><p:cTn id="3" fill="hold"><p:stCondLst><p:cond delay="indefinite"/></p:stCondLst><p:childTnLst>"#,
    r#"<p:par><p:cTn id="4" fill="hold"><p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst>"#,
    r#"<p:par><p:cTn id="5" presetID="1" presetClass="mediacall" presetSubtype="0" fill="hold" nodeType="clickEffect">"#,
    r#"<p:stCondLst><p:cond delay="0"/></p:stCondLst><p:childTnLst><p:cmd type="call" cmd="playFrom(0.0)"><p:cBhvr><p:cTn id="6" dur="1" fill="hold"/><p:tgtEl><p:spTgt spid="4"/></p:tgtEl></p:cBhvr></p:cmd></p:childTnLst></p:cTn></p:par>"#,
    r#"</p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn></p:par></p:childTnLst></p:cTn>"#,
    r#"<p:prevCondLst><p:cond evt="onPrev" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:prevCondLst>"#,
    r#"<p:nextCondLst><p:cond evt="onNext" delay="0"><p:tgtEl><p:sldTgt/></p:tgtEl></p:cond></p:nextCondLst></p:seq>"#
);

/// Media node: vol 60 %, muted, 3 slides, hidden when stopped, looping, rewinding, full screen.
const VIDEO_NODE: &str = concat!(
    r#"<p:video fullScrn="1"><p:cMediaNode vol="60000" mute="1" numSld="3" showWhenStopped="0">"#,
    r#"<p:cTn id="30" repeatCount="indefinite" fill="remove" display="0"><p:stCondLst><p:cond delay="indefinite"/></p:stCondLst></p:cTn>"#,
    r#"<p:tgtEl><p:spTgt spid="4"/></p:tgtEl></p:cMediaNode></p:video>"#
);

fn timing(with_bookmarks: bool) -> String {
    format!(
        r#"<p:timing><p:tnLst><p:par><p:cTn id="1" dur="indefinite" restart="never" nodeType="tmRoot"><p:childTnLst>{MAIN_SEQ}{}{VIDEO_NODE}</p:childTnLst></p:cTn></p:par></p:tnLst></p:timing>"#,
        if with_bookmarks { BMK_SEQ } else { "" }
    )
}

/// The hand-written slide: a text box (20) and a video (4) with every playback option.
fn fixture_slide() -> String {
    format!(
        concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sld {ns} xmlns:mc="http://schemas.openxmlformats.org/markup-compatibility/2006"><p:cSld><p:spTree>"#,
            r#"<p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr/>"#,
            r#"<p:sp><p:nvSpPr><p:cNvPr id="20" name="Chú thích"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x="457200" y="228600"/><a:ext cx="4572000" cy="685800"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>"#,
            r#"<p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang="vi-VN"/><a:t>Ghi chú</a:t></a:r></a:p></p:txBody></p:sp>"#,
            r#"<p:pic><p:nvPicPr><p:cNvPr id="4" name="clip4.mp4"><a:hlinkClick r:id="" action="ppaction://media"/></p:cNvPr><p:cNvPicPr><a:picLocks noChangeAspect="1"/></p:cNvPicPr>"#,
            r#"<p:nvPr><a:videoFile r:link="rId2"/><p:extLst><p:ext uri="{{DAA4B4D4-6D71-4841-9C94-3DE7FCFB9230}}">"#,
            r#"<p14:media xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main" r:embed="rId3">"#,
            r#"<p14:trim st="1500" end="2000.5"/><p14:fade in="1000" out="0.5s"/>"#,
            r#"<p14:bmkLst><p14:bmk name="Mốc 1" time="3000"/><p14:bmk name="Mốc 2" time="4500.25"/></p14:bmkLst>"#,
            r#"<p14:extLst><p:ext uri="{{3AFAAA56-56D3-431D-BCD4-E75A35582382}}"><p1730:tracksInfo xmlns:p1730="http://schemas.microsoft.com/office/powerpoint/2017/3/main" displayLoc="media">"#,
            r#"<p1730:trackLst><p1730:track id="{{6D3C2B1A-0000-4000-8000-000000000001}}" label="Tiếng Việt" lang="vi-VN" r:embed="rId5"/></p1730:trackLst></p1730:tracksInfo></p:ext></p14:extLst>"#,
            r#"</p14:media></p:ext></p:extLst></p:nvPr></p:nvPicPr>"#,
            r#"<p:blipFill><a:blip r:embed="rId4"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>"#,
            r#"<p:spPr><a:xfrm><a:off x="1828800" y="1371600"/><a:ext cx="7315200" cy="4114800"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr></p:pic>"#,
            r#"</p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>"#,
            r#"<mc:AlternateContent><mc:Choice xmlns:p14="http://schemas.microsoft.com/office/powerpoint/2010/main" Requires="p14">{full}</mc:Choice><mc:Fallback>{plain}</mc:Fallback></mc:AlternateContent>"#,
            r#"</p:sld>"#
        ),
        ns = NS,
        full = timing(true),
        plain = timing(false),
    )
}

const VTT: &str = "WEBVTT\n\n00:00:01.000 --> 00:00:03.500\nXin chào\n";

/// The corpus video deck with its slide replaced by the fixture (and a caption part).
fn fixture() -> Vec<u8> {
    let mut pkg = Package::read(
        &corpus("slide-media-video.pptx"),
        &bo_opc::Limits::default(),
    )
    .unwrap();
    pkg.put_part("/ppt/slides/slide1.xml", fixture_slide().into_bytes());
    pkg.add_part("/ppt/media/track1.vtt", "text/vtt", VTT.as_bytes().to_vec())
        .unwrap();
    let mut rels = pkg.relationships("/ppt/slides/slide1.xml").unwrap();
    rels.push(Relationship {
        id: "rId5".into(),
        rel_type: "http://schemas.microsoft.com/office/2017/04/relationships/track".into(),
        target: "../media/track1.vtt".into(),
        mode: TargetMode::Internal,
    });
    pkg.set_relationships("/ppt/slides/slide1.xml", &rels);
    pkg.write().unwrap()
}

fn open(bytes: &[u8]) -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.open(bytes, "media.pptx").unwrap();
    e
}

fn run(e: &mut Engine, json: &str) -> SlideEditResult {
    let r = e
        .slide_command(serde_json::from_str::<SlideCommand>(json).unwrap())
        .unwrap();
    assert!(r.error.is_none(), "{json}: {:?}", r.error);
    r
}

fn media(e: &mut Engine, json: &str) -> SlideEditResult {
    let cmd: MediaCommand =
        serde_json::from_str(json).unwrap_or_else(|err| panic!("{json}: {err}"));
    e.slide_media_command(cmd).unwrap()
}

fn ok(e: &mut Engine, json: &str) -> SlideEditResult {
    let r = media(e, json);
    assert!(r.error.is_none(), "{json}: {:?}", r.error);
    r
}

fn playback(e: &mut Engine) -> MediaPlayback {
    let r = run(e, r#"{"cmd":"state"}"#);
    r.media
        .iter()
        .find(|m| m.kind == "video")
        .map(|m| m.playback.clone())
        .unwrap()
}

fn reopen(e: &Engine) -> Engine {
    open(&e.save_pptx().unwrap())
}

fn slide_xml(e: &Engine) -> String {
    let pkg = Package::read(&e.save_pptx().unwrap(), &bo_opc::Limits::default()).unwrap();
    String::from_utf8(pkg.part("/ppt/slides/slide1.xml").unwrap().data.clone()).unwrap()
}

#[test]
fn every_attribute_is_read_from_the_fixture() {
    let mut e = open(&fixture());
    let p = playback(&mut e);
    // p14:trim / p14:fade (no unit = ms; "0.5s" = 500 ms).
    assert_eq!((p.trim_start_ms, p.trim_end_ms), (1500.0, 2000.5));
    assert_eq!((p.fade_in_ms, p.fade_out_ms), (1000.0, 500.0));
    let b: Vec<(&str, f64)> = p
        .bookmarks
        .iter()
        .map(|b| (b.name.as_str(), b.time_ms))
        .collect();
    assert_eq!(b, [("Mốc 1", 3000.0), ("Mốc 2", 4500.25)]);
    // cMediaNode: vol 60000 = 60 %, mute, numSld 3, showWhenStopped 0; cTn repeatCount
    // indefinite (loop), fill remove (rewind); video fullScrn.
    assert_eq!(p.volume, 60);
    assert!(p.mute && p.looped && p.rewind && p.hide_when_stopped && p.full_screen);
    assert_eq!(p.slides, 3);
    // The play call is the first effect of the first click.
    assert_eq!((p.start, p.play_click), (Some("seq"), Some(1)));
    assert_eq!(p.captions.len(), 1);
    let c = &p.captions[0];
    assert_eq!(
        (c.label.as_str(), c.lang.as_deref(), c.part.as_deref()),
        ("Tiếng Việt", Some("vi-VN"), Some("/ppt/media/track1.vtt"))
    );
    assert_eq!(e.media("/ppt/media/track1.vtt").unwrap(), VTT.as_bytes());
    // The fade-in of shape 20 is started by bookmark "Mốc 1" of clip 4.
    let r = run(&mut e, r#"{"cmd":"show","slide":0}"#);
    let show = r.show.unwrap();
    let fade = show.animations.iter().find(|a| a.shape_id == 20).unwrap();
    assert_eq!(fade.trigger_shape, Some(4));
    assert_eq!(fade.trigger_bookmark.as_deref(), Some("Mốc 1"));
    // The play call is a media command, not a shape animation.
    let play = show.animations.iter().find(|a| a.shape_id == 4).unwrap();
    assert_eq!(play.media_cmd.as_deref(), Some("playFrom(0.0)"));
    assert_eq!(fade.media_cmd, None);
    // Neither the bookmark nor the clip's own play call make a click-trigger button.
    assert_eq!(show.triggers.len(), 0);
}

#[test]
fn fixture_round_trips_unchanged() {
    let mut e = open(&fixture());
    let before = playback(&mut e);
    let xml = slide_xml(&e);
    // The timing is written back as read (still in mc:AlternateContent with its Fallback).
    assert!(xml.contains("<mc:AlternateContent"), "{xml}");
    assert!(
        xml.contains(r#"<p14:bmkTgt spid="4" bmkName="Mốc 1"/>"#),
        "{xml}"
    );
    assert!(xml.contains(r#"<p:video fullScrn="1">"#), "{xml}");
    assert!(xml.contains(r#"<p14:fade in="1000" out="0.5s"/>"#), "{xml}");
    let mut back = reopen(&e);
    assert_eq!(playback(&mut back), before);
}

#[test]
fn playback_edits_are_undoable_and_saved() {
    let mut e = open(&fixture());
    run(&mut e, r#"{"cmd":"select","ids":[4]}"#);
    let original = playback(&mut e);
    ok(
        &mut e,
        r#"{"cmd":"mediaPlayback","trimStartMs":250,"trimEndMs":0,"fadeInMs":0,"fadeOutMs":1200,"volume":25,"mute":false,"slides":1,"loop":false,"rewind":false,"hideWhenStopped":false,"fullScreen":false,"start":"auto"}"#,
    );
    let p = playback(&mut e);
    assert_eq!((p.trim_start_ms, p.trim_end_ms), (250.0, 0.0));
    assert_eq!((p.fade_in_ms, p.fade_out_ms), (0.0, 1200.0));
    assert_eq!(p.volume, 25);
    assert!(!(p.mute || p.looped || p.rewind || p.hide_when_stopped || p.full_screen));
    assert_eq!(p.slides, 1);
    assert_eq!((p.start, p.play_click), (Some("auto"), Some(0)));
    // Saved, reopened: the same options; the XML holds what the spec says.
    let xml = slide_xml(&e);
    assert!(xml.contains(r#"<p14:trim st="250"/>"#), "{xml}");
    assert!(xml.contains(r#"<p14:fade out="1200"/>"#), "{xml}");
    assert!(xml.contains(r#"<p:cMediaNode vol="25000">"#), "{xml}");
    assert!(xml.contains(r#"cmd="playFrom(0.0)""#), "{xml}");
    let mut back = reopen(&e);
    assert_eq!(playback(&mut back), p);
    // The bookmark trigger survived the regenerated timing.
    let r = run(&mut back, r#"{"cmd":"show","slide":0}"#);
    let fade = r
        .show
        .unwrap()
        .animations
        .into_iter()
        .find(|a| a.shape_id == 20)
        .unwrap();
    assert_eq!(fade.trigger_bookmark.as_deref(), Some("Mốc 1"));
    // One undo restores everything.
    run(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(playback(&mut e), original);
    run(&mut e, r#"{"cmd":"redo"}"#);
    assert_eq!(playback(&mut e), p);
}

#[test]
fn bookmarks_are_added_removed_and_trigger_effects() {
    let mut e = open(&fixture());
    run(&mut e, r#"{"cmd":"select","ids":[4]}"#);
    ok(&mut e, r#"{"cmd":"mediaBookmarkAdd","timeMs":1000}"#);
    let p = playback(&mut e);
    // Numbered name, kept in time order.
    let names: Vec<&str> = p.bookmarks.iter().map(|b| b.name.as_str()).collect();
    assert_eq!(names, ["Bookmark 1", "Mốc 1", "Mốc 2"]);
    // Same time or same name: refused ([MS-PPTX] §2.3.3.11 uniqueness).
    let r = media(&mut e, r#"{"cmd":"mediaBookmarkAdd","timeMs":3000}"#);
    assert_eq!(r.error.as_deref(), Some("bookmarkExists"));
    let r = media(
        &mut e,
        r#"{"cmd":"mediaBookmarkAdd","timeMs":7000,"name":"Mốc 2"}"#,
    );
    assert_eq!(r.error.as_deref(), Some("bookmarkExists"));
    // Retarget the fade to the new bookmark.
    let show = run(&mut e, r#"{"cmd":"show","slide":0}"#).show.unwrap();
    let idx = show
        .animations
        .iter()
        .find(|a| a.shape_id == 20)
        .unwrap()
        .index;
    ok(
        &mut e,
        &format!(
            r#"{{"cmd":"mediaBookmarkTrigger","index":{idx},"media":4,"bookmark":"Bookmark 1"}}"#
        ),
    );
    let r = media(
        &mut e,
        &format!(r#"{{"cmd":"mediaBookmarkTrigger","index":{idx},"media":4,"bookmark":"nope"}}"#),
    );
    assert_eq!(r.error.as_deref(), Some("noBookmark"));
    let mut back = reopen(&e);
    let show = run(&mut back, r#"{"cmd":"show","slide":0}"#).show.unwrap();
    let fade = show.animations.iter().find(|a| a.shape_id == 20).unwrap();
    assert_eq!(fade.trigger_bookmark.as_deref(), Some("Bookmark 1"));
    // Written in mc:AlternateContent; the Fallback has no p14 bookmark target.
    let xml = slide_xml(&e);
    let fb = &xml[xml.find("<mc:Fallback>").unwrap()..];
    assert!(
        !fb[..fb.find("</mc:Fallback>").unwrap()].contains("bmkTgt"),
        "{xml}"
    );
    assert!(
        xml.contains(r#"<p14:bmkTgt spid="4" bmkName="Bookmark 1"/>"#),
        "{xml}"
    );
    // Removing the bookmark: the effect goes back to a click on the clip.
    ok(
        &mut e,
        r#"{"cmd":"mediaBookmarkRemove","name":"Bookmark 1"}"#,
    );
    let p = playback(&mut e);
    assert_eq!(p.bookmarks.len(), 2);
    let show = run(&mut e, r#"{"cmd":"show","slide":0}"#).show.unwrap();
    let fade = show.animations.iter().find(|a| a.shape_id == 20).unwrap();
    assert_eq!(
        (fade.trigger_shape, fade.trigger_bookmark.as_deref()),
        (Some(4), None)
    );
    // Undo puts the bookmark and the trigger back.
    run(&mut e, r#"{"cmd":"undo"}"#);
    let show = run(&mut e, r#"{"cmd":"show","slide":0}"#).show.unwrap();
    let fade = show.animations.iter().find(|a| a.shape_id == 20).unwrap();
    assert_eq!(fade.trigger_bookmark.as_deref(), Some("Bookmark 1"));
}

#[test]
fn captions_are_inserted_and_removed() {
    let mut e = open(&fixture());
    run(&mut e, r#"{"cmd":"select","ids":[4]}"#);
    let r = media(
        &mut e,
        r#"{"cmd":"mediaCaptionAdd","text":"1\n00:00:01,000 --> 00:00:02,000\nSRT"}"#,
    );
    assert_eq!(r.error.as_deref(), Some("badCaptions"));
    let text = "WEBVTT\n\n00:02.000 --> 00:04.000\nHello\n";
    ok(
        &mut e,
        &serde_json::json!({"cmd":"mediaCaptionAdd","text":text,"label":"English","lang":"en-US"})
            .to_string(),
    );
    let p = playback(&mut e);
    assert_eq!(p.captions.len(), 2);
    let en = p
        .captions
        .iter()
        .find(|c| c.label == "English")
        .unwrap()
        .clone();
    assert_eq!(en.lang.as_deref(), Some("en-US"));
    assert_eq!(
        e.media(en.part.as_deref().unwrap()).unwrap(),
        text.as_bytes()
    );
    let mut back = reopen(&e);
    let q = playback(&mut back);
    assert_eq!(q.captions, p.captions);
    let xml = slide_xml(&e);
    assert_eq!(xml.matches("<p1730:track ").count(), 2, "{xml}");
    // Remove the first track: its part and relationship go away.
    let vi = p
        .captions
        .iter()
        .find(|c| c.label == "Tiếng Việt")
        .unwrap()
        .clone();
    ok(
        &mut e,
        &format!(r#"{{"cmd":"mediaCaptionRemove","track":"{}"}}"#, vi.id),
    );
    let p = playback(&mut e);
    assert_eq!(p.captions.len(), 1);
    assert!(e.media("/ppt/media/track1.vtt").is_none());
    run(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(playback(&mut e).captions.len(), 2);
    assert!(e.media("/ppt/media/track1.vtt").is_some());
}

fn child<'a>(el: &'a XmlElement, name: &str) -> &'a XmlElement {
    el.child(name)
        .unwrap_or_else(|| panic!("{} has no {name}", el.name))
}

fn kids(el: &XmlElement) -> Vec<&XmlElement> {
    el.children
        .iter()
        .filter_map(|c| match c {
            XmlNode::Element(e) => Some(e),
            XmlNode::Text(_) => None,
        })
        .collect()
}

/// The root time node's children of the generated timing of slide 0.
fn root_children(e: &Engine) -> Vec<XmlElement> {
    let t = e.presentation().unwrap().slides[0].timing.clone().unwrap();
    let x = bo_pptx::anim::write_timing(&t).unwrap();
    let root = child(child(child(&x.xml, "p:tnLst"), "p:par"), "p:cTn");
    assert_eq!(root.attr("nodeType"), Some("tmRoot"));
    kids(child(root, "p:childTnLst"))
        .into_iter()
        .cloned()
        .collect()
}

/// The media call of an effect `p:par`: (nodeType, presetClass, cmd, target).
fn call_of(par: &XmlElement) -> (String, String, String, String) {
    let ctn = child(par, "p:cTn");
    let cmd = child(child(ctn, "p:childTnLst"), "p:cmd");
    let spid = child(child(child(cmd, "p:cBhvr"), "p:tgtEl"), "p:spTgt");
    (
        ctn.attr("nodeType").unwrap().into(),
        ctn.attr("presetClass").unwrap().into(),
        cmd.attr("cmd").unwrap().into(),
        spid.attr("spid").unwrap().into(),
    )
}

#[test]
fn start_modes_generate_the_timing_tree() {
    // A fresh clip on a deck without timing.
    let mut e = open(&corpus("slide-text-bullets.pptx"));
    let mut mp4 = b"\x00\x00\x00\x18ftypmp42\x00\x00\x00\x00mp42isom".to_vec();
    mp4.extend(std::iter::repeat_n(0u8, 32));
    let r = e.slide_insert_picture(mp4, "phim.mp4").unwrap();
    let id = r.media[0].id;
    assert_eq!(r.media[0].playback.start, None);
    assert_eq!(r.media[0].playback.volume, 100);

    // In the click sequence: tmRoot → mainSeq → click par (indefinite) → group par (0) →
    // effect par (clickEffect, mediacall, playFrom(0.0) on the clip); then the video node.
    ok(&mut e, r#"{"cmd":"mediaPlayback","start":"seq"}"#);
    let kids_ = root_children(&e);
    assert_eq!(kids_.len(), 2);
    assert_eq!(kids_[0].name, "p:seq");
    let main = child(&kids_[0], "p:cTn");
    assert_eq!(main.attr("nodeType"), Some("mainSeq"));
    let click = kids(child(main, "p:childTnLst"))[0];
    let click_ctn = child(click, "p:cTn");
    let delay = |c: &XmlElement| {
        child(child(c, "p:stCondLst"), "p:cond")
            .attr("delay")
            .unwrap()
            .to_owned()
    };
    assert_eq!(delay(click_ctn), "indefinite");
    let group = kids(child(click_ctn, "p:childTnLst"))[0];
    assert_eq!(delay(child(group, "p:cTn")), "0");
    let effect = kids(child(child(group, "p:cTn"), "p:childTnLst"))[0];
    assert_eq!(
        call_of(effect),
        (
            "clickEffect".into(),
            "mediacall".into(),
            "playFrom(0.0)".into(),
            id.to_string()
        )
    );
    // §19.5 p:video → p:cMediaNode (vol 100000 = 100 %) → cTn started indefinitely + spTgt.
    assert_eq!(kids_[1].name, "p:video");
    let node = child(&kids_[1], "p:cMediaNode");
    assert_eq!(node.attr("vol"), Some("100000"));
    assert_eq!(delay(child(node, "p:cTn")), "indefinite");
    assert_eq!(
        child(child(node, "p:tgtEl"), "p:spTgt").attr("spid"),
        Some(id.to_string().as_str())
    );
    let p = playback(&mut e);
    assert_eq!((p.start, p.play_click), (Some("seq"), Some(1)));

    // Automatically: the first click group starts at once (delay 0), after-previous effect.
    ok(&mut e, r#"{"cmd":"mediaPlayback","start":"auto"}"#);
    let kids_ = root_children(&e);
    let main = child(&kids_[0], "p:cTn");
    let click_ctn = child(kids(child(main, "p:childTnLst"))[0], "p:cTn");
    assert_eq!(delay(click_ctn), "0");
    let group = kids(child(click_ctn, "p:childTnLst"))[0];
    let effect = kids(child(child(group, "p:cTn"), "p:childTnLst"))[0];
    assert_eq!(call_of(effect).0, "afterEffect");
    assert_eq!(playback(&mut e).play_click, Some(0));

    // On a click on the clip: an interactive sequence started by onClick on the clip,
    // toggling pause; no main sequence left.
    ok(&mut e, r#"{"cmd":"mediaPlayback","start":"click"}"#);
    let kids_ = root_children(&e);
    assert_eq!(kids_.len(), 2);
    let inter = child(&kids_[0], "p:cTn");
    assert_eq!(inter.attr("nodeType"), Some("interactiveSeq"));
    let cond = child(child(inter, "p:stCondLst"), "p:cond");
    assert_eq!(cond.attr("evt"), Some("onClick"));
    assert_eq!(
        child(child(cond, "p:tgtEl"), "p:spTgt").attr("spid"),
        Some(id.to_string().as_str())
    );
    let click_ctn = child(kids(child(inter, "p:childTnLst"))[0], "p:cTn");
    let group = kids(child(click_ctn, "p:childTnLst"))[0];
    let effect = kids(child(child(group, "p:cTn"), "p:childTnLst"))[0];
    assert_eq!(call_of(effect).2, "togglePause");
    let p = playback(&mut e);
    assert_eq!((p.start, p.play_click), (Some("click"), None));
    // The clip handles its own click: no trigger button over it.
    let r = run(&mut e, r#"{"cmd":"show","slide":0}"#);
    assert_eq!(r.show.unwrap().triggers.len(), 0);

    // None: no start effect, the media node stays.
    ok(&mut e, r#"{"cmd":"mediaPlayback","start":"none"}"#);
    let kids_ = root_children(&e);
    assert_eq!(kids_.len(), 1);
    assert_eq!(kids_[0].name, "p:video");
    let r = media(&mut e, r#"{"cmd":"mediaPlayback","start":"sometimes"}"#);
    assert_eq!(r.error.as_deref(), Some("badMediaStart"));
    let r = media(&mut e, r#"{"cmd":"mediaPlayback","volume":150}"#);
    assert_eq!(r.error.as_deref(), Some("badVolume"));

    // Time node ids stay unique after the edits.
    let t = e.presentation().unwrap().slides[0].timing.clone().unwrap();
    let x = bo_pptx::anim::write_timing(&t).unwrap();
    let mut ids = Vec::new();
    collect_ids(&x.xml, &mut ids);
    let n = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), n);
}

fn collect_ids(el: &XmlElement, out: &mut Vec<String>) {
    if el.name == "p:cTn" {
        out.extend(el.attr("id").map(str::to_owned));
    }
    for c in kids(el) {
        collect_ids(c, out);
    }
}

#[test]
fn json_bridge_routes_media_commands() {
    let mut e = open(&fixture());
    let r = e
        .slide_command_json(r#"{"cmd":"mediaPlayback","id":4,"volume":40}"#)
        .unwrap();
    let v: serde_json::Value = serde_json::from_str(&r).unwrap();
    assert!(v["error"].is_null(), "{r}");
    let clip = &v["media"][0]["playback"];
    assert_eq!(clip["volume"], 40);
    assert_eq!(clip["loop"], true);
    // No clip selected or named: refused.
    let r = e
        .slide_command_json(r#"{"cmd":"mediaBookmarkAdd","timeMs":10}"#)
        .unwrap();
    assert!(r.contains(r#""error":"noMedia""#), "{r}");
}

/// Decks with every playback option, written to `$BO_MEDIA_SAMPLES` (when set) for
/// `bo-cli validate --openxml require`.
#[test]
fn edited_decks_for_the_validator() {
    let mut e = open(&fixture());
    run(&mut e, r#"{"cmd":"select","ids":[4]}"#);
    ok(
        &mut e,
        r#"{"cmd":"mediaBookmarkAdd","timeMs":1250,"name":"Thêm"}"#,
    );
    ok(
        &mut e,
        r#"{"cmd":"mediaPlayback","fadeInMs":0,"trimEndMs":0,"start":"click","volume":75}"#,
    );
    ok(
        &mut e,
        &serde_json::json!({"cmd":"mediaCaptionAdd","text":VTT,"label":"Hai"}).to_string(),
    );
    let fixture_edited = e.save_pptx().unwrap();

    let mut f = open(&corpus("slide-text-bullets.pptx"));
    let mut mp4 = b"\x00\x00\x00\x18ftypmp42\x00\x00\x00\x00mp42isom".to_vec();
    mp4.extend(std::iter::repeat_n(0u8, 32));
    f.slide_insert_picture(mp4, "phim.mp4").unwrap();
    ok(
        &mut f,
        r#"{"cmd":"mediaPlayback","trimStartMs":500,"trimEndMs":250,"fadeInMs":1000,"fadeOutMs":750,"start":"seq","volume":50,"mute":true,"slides":999,"loop":true,"rewind":true,"hideWhenStopped":true,"fullScreen":true}"#,
    );
    ok(&mut f, r#"{"cmd":"mediaBookmarkAdd","timeMs":2000}"#);
    ok(
        &mut f,
        &serde_json::json!({"cmd":"mediaCaptionAdd","text":VTT,"lang":"vi-VN"}).to_string(),
    );
    let fresh = f.save_pptx().unwrap();
    for bytes in [&fixture_edited, &fresh] {
        assert!(open(bytes).presentation().is_some());
    }
    if let Some(dir) = std::env::var_os("BO_MEDIA_SAMPLES") {
        let dir = PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("media-fixture.pptx"), fixture()).unwrap();
        std::fs::write(dir.join("media-fixture-edited.pptx"), &fixture_edited).unwrap();
        std::fs::write(dir.join("media-fresh.pptx"), &fresh).unwrap();
    }
}

#[test]
fn deleting_a_clip_drops_its_media_node_and_bookmark_triggers() {
    let mut e = open(&fixture());
    run(&mut e, r#"{"cmd":"select","ids":[4]}"#);
    run(&mut e, r#"{"cmd":"delete"}"#);
    let t = e.presentation().unwrap().slides[0].timing.clone().unwrap();
    assert_eq!(t.others.len(), 0);
    let fade = t.animations.iter().find(|a| a.shape_id == 20).unwrap();
    assert_eq!(
        (fade.trigger_shape, fade.trigger_bookmark.as_deref()),
        (None, None)
    );
    // Saved without the p14 alternate content (no bookmark target left).
    let xml = slide_xml(&e);
    assert!(
        !xml.contains("bmkTgt") && !xml.contains("<p:video"),
        "{xml}"
    );
    run(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(playback(&mut e).slides, 3);
}
