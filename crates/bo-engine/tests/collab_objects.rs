//! Live collaboration of the objects and settings added after Phase 6
//! (CORE-COLLAB-001, audit in `docs/collab-coverage.md`): two engines, each
//! with its own `bo-collab` replica, exchange real y-sync frames; a change made
//! by one participant reaches the other and both end with the same model.
//!
//! * BlueDoc: page colour / theme, hyphenation, custom XML parts, editing
//!   restriction, an embedded workbook edited in place (its part keeps its name);
//! * BlueSheet: pictures in cells, a theme applied, threaded-comment persons,
//!   workbook protection;
//! * BlueSlide: comment parts rewritten in place, slide size.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_collab::{Change, Session};
use bo_engine::sheet::SheetEngine;
use bo_engine::{CollabChange, Engine};
use serde_json::{json, Value};
use std::path::PathBuf;

fn to_replica(c: &CollabChange) -> Change {
    serde_json::from_value(serde_json::to_value(c).unwrap()).unwrap()
}

fn to_engine(c: &Change) -> CollabChange {
    serde_json::from_value(serde_json::to_value(c).unwrap()).unwrap()
}

fn b64(b: &[u8]) -> String {
    use base64::Engine as _;
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn corpus(dir: &str, name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus")
        .join(dir)
        .join(name);
    std::fs::read(p).unwrap()
}

/// Runs `f` with a large stack: debug builds keep a slide master's text
/// styles on the stack while (de)serializing them.
fn big_stack(f: impl FnOnce() + Send) {
    std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn_scoped(s, f)
            .unwrap()
            .join()
            .unwrap();
    });
}

// ---------------------------------------------------------------------------
// BlueDoc / BlueSlide (one engine type)
// ---------------------------------------------------------------------------

struct Peer {
    engine: Engine,
    replica: Session,
    slides: bool,
}

impl Peer {
    fn doc(id: u64, text: &str) -> Peer {
        let mut engine = Engine::with_bundled_fonts();
        engine
            .load(bo_engine::plain_text_document(text), "chung.docx")
            .unwrap();
        Peer {
            engine,
            replica: Session::with_client_id(id),
            slides: false,
        }
    }

    fn deck(id: u64, name: &str) -> Peer {
        let mut engine = Engine::with_bundled_fonts();
        engine.open(&corpus("slide", name), name).unwrap();
        Peer {
            engine,
            replica: Session::with_client_id(id),
            slides: true,
        }
    }

    fn start(&mut self, seed: bool) {
        let id = self.replica.client_id();
        let c = if self.slides {
            self.engine.slide_collab_start(seed, id).unwrap()
        } else {
            self.engine.collab_start(seed, id).unwrap()
        };
        if seed {
            self.replica.seed(&to_replica(&c)).unwrap();
        }
    }

    /// Runs a command (BlueDoc or BlueSlide) and shares its change.
    fn cmd(&mut self, cmd: &Value) -> Value {
        let json = cmd.to_string();
        let out = if self.slides {
            self.engine.slide_command_json(&json).unwrap()
        } else {
            self.engine.command_json(&json).unwrap()
        };
        let r: Value = serde_json::from_str(&out).unwrap();
        assert!(r.get("error").is_none_or(Value::is_null), "{json}: {r}");
        let c: CollabChange = serde_json::from_value(r["collab"]["change"].clone()).unwrap();
        self.replica.apply_local(&to_replica(&c)).unwrap();
        r
    }

    fn pull(&mut self) {
        let ch = self.replica.take_remote();
        if ch.is_empty() {
            return;
        }
        let c = to_engine(&ch);
        let back = if self.slides {
            let r = self.engine.slide_collab_apply(&c).unwrap();
            assert!(r.error.is_none(), "{:?}", r.error);
            r.collab.unwrap().change
        } else {
            let r = self.engine.collab_apply(&c).unwrap();
            assert!(r.error.is_none(), "{:?}", r.error);
            r.collab.unwrap().change
        };
        // Applying a remote change shares nothing back.
        assert!(back.is_empty(), "echo: {back:?}");
    }
}

fn send(from: &mut Peer, to: &mut Peer) {
    for f in from.replica.drain_outbox() {
        for r in to.replica.receive(&f).unwrap() {
            from.replica.receive(&r).unwrap();
        }
    }
}

fn connect(a: &mut Peer, b: &mut Peer) {
    let s = a.replica.start().unwrap();
    for r in b.replica.receive(&s).unwrap() {
        a.replica.receive(&r).unwrap();
    }
    let s = b.replica.start().unwrap();
    for r in a.replica.receive(&s).unwrap() {
        b.replica.receive(&r).unwrap();
    }
}

fn settle(a: &mut Peer, b: &mut Peer) {
    for _ in 0..4 {
        send(a, b);
        send(b, a);
        a.pull();
        b.pull();
    }
    send(a, b);
    send(b, a);
}

fn doc_pair(text: &str) -> (Peer, Peer) {
    let mut a = Peer::doc(1, text);
    let mut b = Peer::doc(2, "khác hẳn");
    a.start(true);
    b.start(false);
    connect(&mut a, &mut b);
    settle(&mut a, &mut b);
    (a, b)
}

fn deck_pair(name: &str) -> (Peer, Peer) {
    let mut a = Peer::deck(1, name);
    let mut b = Peer::deck(2, name);
    a.start(true);
    b.start(false);
    connect(&mut a, &mut b);
    settle(&mut a, &mut b);
    (a, b)
}

fn doc_settings(p: &Peer) -> Value {
    let d = p.engine.document().unwrap();
    json!({
        "design": d.design,
        "fonts": d.theme_fonts,
        "hyph": d.hyphenation,
        "cx": d.custom_xml,
        "protection": d.protection,
    })
}

#[test]
fn doc_settings_set_at_once_by_two_peers_converge() {
    let (mut a, mut b) = doc_pair("Báo cáo quý");
    // Concurrent changes of different settings: both survive.
    a.cmd(&json!({"cmd":"setPageColor","color":"FFF2CC"}));
    b.cmd(&json!({"cmd":"setHyphenation","hyphenation":{"auto":true,"zone":360,"consecutiveLimit":2,"noCaps":true}}));
    a.cmd(&json!({"cmd":"customXmlAdd","xml":"<hopdong xmlns=\"urn:bluestar:hd\"><so>12/2026</so></hopdong>"}));
    settle(&mut a, &mut b);
    assert_eq!(doc_settings(&a), doc_settings(&b));
    let d = b.engine.document().unwrap();
    assert!(d.design.background.is_some(), "page colour reached B");
    assert!(d.hyphenation.auto, "hyphenation reached A and B");
    assert_eq!(d.custom_xml.len(), 1, "custom XML part reached B");
    // A theme applied by B (theme part + fonts) reaches A.
    b.cmd(&json!({"cmd":"applyThemeFile","file":b64(&corpus("slide","slide-notes-basic.pptx"))}));
    settle(&mut a, &mut b);
    assert!(a.engine.document().unwrap().design.theme_xml.is_some());
    assert_eq!(doc_settings(&a), doc_settings(&b));
    // The editing restriction set by A applies on B; stopping it too.
    a.cmd(&json!({"cmd":"protectDocument","edit":"readOnly"}));
    settle(&mut a, &mut b);
    assert_eq!(
        b.engine
            .document()
            .unwrap()
            .protection
            .as_ref()
            .map(|p| (p.edit.as_str(), p.enforced)),
        Some(("readOnly", true))
    );
    a.cmd(&json!({"cmd":"unprotectDocument"}));
    settle(&mut a, &mut b);
    assert_eq!(doc_settings(&a), doc_settings(&b));
    assert!(!b
        .engine
        .document()
        .unwrap()
        .protection
        .as_ref()
        .is_some_and(|p| p.enforced));
}

#[test]
fn doc_settings_reach_a_late_joiner() {
    let mut a = Peer::doc(1, "Thư mời");
    a.start(true);
    a.cmd(&json!({"cmd":"setPageColor","color":"DDEBF7"}));
    a.cmd(&json!({"cmd":"customXmlAdd","xml":"<a xmlns=\"urn:x\">1</a>"}));
    a.cmd(&json!({"cmd":"protectDocument","edit":"comments"}));
    let mut b = Peer::doc(2, "khác");
    b.start(false);
    connect(&mut a, &mut b);
    settle(&mut a, &mut b);
    assert_eq!(doc_settings(&a), doc_settings(&b));
}

/// A small workbook built with BlueSheet: Sheet1!A1:B2.
fn workbook() -> Vec<u8> {
    let mut s = SheetEngine::with_bundled_fonts();
    s.new_workbook("b.xlsx").unwrap();
    s.set_input(0, 0, 0, "Năm").unwrap();
    s.set_input(0, 0, 1, "Doanh thu").unwrap();
    s.set_input(0, 1, 0, "2026").unwrap();
    s.set_input(0, 1, 1, "=10*12").unwrap();
    s.save("xlsx", 0).unwrap()
}

#[test]
fn an_embedded_workbook_edited_in_place_reaches_the_other_peer() {
    let (mut a, mut b) = doc_pair("");
    a.cmd(&json!({"cmd":"oleInsert","data":b64(&workbook()),"range":"Sheet1!A1:B2"}));
    settle(&mut a, &mut b);
    let media = |p: &Peer| p.engine.document().unwrap().media.clone();
    assert_eq!(media(&a), media(&b));
    // The edit rewrites the embedded package under the same part name.
    a.cmd(
        &json!({"cmd":"setSelection","selection":{"anchor":{"p":0,"o":0},"focus":{"p":0,"o":0}}}),
    );
    let before = media(&a);
    a.cmd(&json!({"cmd":"oleEdit","cells":[["Năm","Doanh thu","Lãi"],["2026","=10*12","=B2/4"]]}));
    let after = media(&a);
    let rewritten: Vec<&String> = after
        .iter()
        .filter(|(k, v)| before.get(*k).is_some_and(|o| o != *v))
        .map(|(k, _)| k)
        .collect();
    assert!(!rewritten.is_empty(), "the package part keeps its name");
    settle(&mut a, &mut b);
    assert_eq!(media(&a), media(&b));
    let v: Value = serde_json::from_str(
        &b.engine
            .command_json(r#"{"cmd":"oleInfo","cells":true}"#)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(v["ole"]["cells"][1][2], "=B2/4", "{v}");
}

impl Peer {
    fn share(&mut self, r: &bo_engine::edit::EditResult) {
        let c = r.collab.as_ref().unwrap().change.clone();
        self.replica.apply_local(&to_replica(&c)).unwrap();
    }
}

/// Pictures in the body (by media part name) of a peer.
fn pictures(p: &Peer) -> Vec<(String, usize)> {
    let d = p.engine.document().unwrap();
    let mut v = Vec::new();
    for s in &d.sections {
        for b in &s.blocks {
            if let bo_model::Block::Paragraph(par) = b {
                for r in &par.runs {
                    for c in &r.content {
                        if let bo_model::RunContent::Image(i) = c {
                            v.push((
                                i.media.clone(),
                                d.media.get(&i.media).map_or(0, |m| m.len()),
                            ));
                        }
                    }
                }
            }
        }
    }
    v
}

// source: ECMA-376 Part 2, §9.1.1 part names (media parts added at once on two replicas)
#[test]
fn doc_pictures_and_smartart_added_at_once_keep_their_own_parts() {
    let (mut a, mut b) = doc_pair("Một\nHai");
    // Both insert a picture at once: two parts, two pictures, the same on both sides.
    a.cmd(
        &json!({"cmd":"setSelection","selection":{"anchor":{"p":0,"o":3},"focus":{"p":0,"o":3}}}),
    );
    b.cmd(
        &json!({"cmd":"setSelection","selection":{"anchor":{"p":1,"o":3},"focus":{"p":1,"o":3}}}),
    );
    let r = a.engine.insert_image(PNG_2X1.to_vec(), "a.png").unwrap();
    a.share(&r);
    let mut big = PNG_2X1.to_vec();
    big.extend_from_slice(&[0; 16]);
    let r = b.engine.insert_image(big, "b.png").unwrap();
    b.share(&r);
    settle(&mut a, &mut b);
    let (pa, pb) = (pictures(&a), pictures(&b));
    assert_eq!(pa, pb);
    assert_eq!(pa.len(), 2);
    assert_ne!(pa[0].0, pa[1].0, "distinct part names");
    assert_ne!(pa[0].1, pa[1].1, "each keeps its own bytes");
    assert_eq!(
        a.engine.document().unwrap().media,
        b.engine.document().unwrap().media
    );
    // SmartArt inserted by A, its text edited by B: parts and frame converge.
    a.cmd(
        &json!({"cmd":"setSelection","selection":{"anchor":{"p":1,"o":0},"focus":{"p":1,"o":0}}}),
    );
    a.cmd(&json!({"cmd":"insertSmartArt","layout":"process","items":[{"text":"Một","level":0},{"text":"Hai","level":0}]}));
    settle(&mut a, &mut b);
    assert_eq!(pictures(&a), pictures(&b));
    let frame = |p: &Peer| {
        pictures(p)
            .into_iter()
            .find(|(m, _)| m.contains("/diagrams/"))
            .unwrap()
            .0
    };
    let media = frame(&b);
    let at = b
        .engine
        .document()
        .unwrap()
        .sections
        .iter()
        .flat_map(|s| s.blocks.iter())
        .position(|bl| match bl {
            bo_model::Block::Paragraph(p) => p.runs.iter().any(|r| {
                r.content
                    .iter()
                    .any(|c| matches!(c, bo_model::RunContent::Image(i) if i.media == media))
            }),
            _ => false,
        })
        .unwrap();
    b.cmd(
        &json!({"cmd":"setSelection","selection":{"anchor":{"p":at,"o":0},"focus":{"p":at,"o":0}}}),
    );
    b.cmd(&json!({"cmd":"setSmartArt","layout":"process","items":[{"text":"Một","level":0},{"text":"Hai","level":0},{"text":"Ba","level":0}]}));
    settle(&mut a, &mut b);
    assert_eq!(pictures(&a), pictures(&b));
    assert_eq!(
        a.engine.document().unwrap().media,
        b.engine.document().unwrap().media
    );
    let info = a.engine.doc_smartart().unwrap();
    assert_eq!(info.items.len(), 3, "B's text reached A");
}

#[test]
fn slide_comment_parts_rewritten_in_place_converge() {
    big_stack(|| {
        let (mut a, mut b) = deck_pair("slide-notes-basic.pptx");
        a.cmd(&json!({"cmd":"addComment","text":"Sửa tiêu đề","author":"An"}));
        settle(&mut a, &mut b);
        assert_eq!(
            a.engine.presentation().unwrap().parts,
            b.engine.presentation().unwrap().parts
        );
        // A second thread on the same slide rewrites the comment part.
        b.cmd(&json!({"cmd":"addComment","text":"Đồng ý","author":"Bình"}));
        settle(&mut a, &mut b);
        assert_eq!(
            a.engine.presentation().unwrap(),
            b.engine.presentation().unwrap()
        );
    });
}

#[test]
fn slide_size_change_reaches_the_other_peer() {
    big_stack(|| {
        let (mut a, mut b) = deck_pair("slide-notes-basic.pptx");
        a.cmd(&json!({"cmd":"setSlideSize","cx":9_144_000,"cy":6_858_000,"kind":"screen4x3","mode":"scale"}));
        settle(&mut a, &mut b);
        let p = b.engine.presentation().unwrap();
        assert_eq!((p.slide_size.cx, p.slide_size.cy), (9_144_000, 6_858_000));
        assert_eq!(
            a.engine.presentation().unwrap(),
            b.engine.presentation().unwrap()
        );
    });
}

#[test]
fn slide_design_idea_reaches_the_other_peer() {
    // CORE-DESIGN-001: moved / cropped shapes and the new accent shapes converge.
    big_stack(|| {
        let (mut a, mut b) = deck_pair("slide-notes-basic.pptx");
        a.cmd(&json!({"cmd":"goTo","slide":1}));
        let r = a.cmd(&json!({"cmd":"designIdeas"}));
        let id = r["designIdeas"]["ideas"][1]["id"]
            .as_str()
            .unwrap()
            .to_owned();
        a.cmd(&json!({"cmd":"designApply","idea":id}));
        settle(&mut a, &mut b);
        let accents = |p: &Peer| {
            p.engine.presentation().unwrap().slides[1]
                .data
                .shapes
                .iter()
                .filter(|n| {
                    n.nv()
                        .is_some_and(|nv| nv.name.starts_with("Design Accent"))
                })
                .count()
        };
        assert!(accents(&b) > 0);
        assert_eq!(
            a.engine.presentation().unwrap().slides[1],
            b.engine.presentation().unwrap().slides[1]
        );
    });
}

#[test]
fn slide_presence_names_the_current_slide() {
    big_stack(|| {
        let (mut a, _b) = deck_pair("slide-edit-slides.pptx");
        let r = a.cmd(&json!({"cmd":"goTo","slide":1}));
        let key = r["collab"]["slide"].as_str().unwrap().to_owned();
        let id = a.engine.presentation().unwrap().slides[1].slide_id;
        assert_eq!(key, id.to_string());
        let peers = a.engine.slide_collab_peers(&[format!("{key}/")]).unwrap();
        assert_eq!(peers[0].as_ref().map(|p| p.slide), Some(1));
    });
}

#[test]
fn doc_presence_has_the_paragraph_band() {
    let (mut a, _b) = doc_pair("Một\nHai dòng của đoạn thứ hai\nBa");
    let _ = a.cmd(
        &json!({"cmd":"setSelection","selection":{"anchor":{"p":1,"o":2},"focus":{"p":1,"o":2}}}),
    );
    let runs: String = a.engine.collab_runs().into_iter().map(|r| r.0).collect();
    let start = runs.find("Hai").unwrap();
    let off = u32::try_from(runs[..start].encode_utf16().count() + 2).unwrap();
    let peers = a.engine.collab_peers(&[(off, off)]).unwrap();
    let caret = peers[0].caret.unwrap();
    let band = peers[0].para.unwrap();
    assert_eq!(band.page, caret.page);
    assert!(band.y <= caret.y && band.y + band.h >= caret.y + caret.h - 0.01);
    assert!(band.x <= caret.x);
}

// ---------------------------------------------------------------------------
// BlueSheet
// ---------------------------------------------------------------------------

struct SheetPeer {
    engine: SheetEngine,
    replica: Session,
}

impl SheetPeer {
    fn share(&mut self) {
        let c = self.engine.collab_local().unwrap();
        self.replica.apply_local(&to_replica(&c)).unwrap();
    }

    fn exec(&mut self, cmd: &Value) -> Value {
        let r: Value = serde_json::from_str(&self.engine.exec_json(&cmd.to_string()).unwrap())
            .unwrap_or(Value::Null);
        self.share();
        r
    }

    fn pull(&mut self) {
        let ch = self.replica.take_remote();
        if ch.is_empty() {
            return;
        }
        self.engine.collab_apply(&to_engine(&ch)).unwrap();
        let back = self.engine.collab_local().unwrap();
        assert!(back.is_empty(), "echo: {back:?}");
    }
}

fn sheet_send(from: &mut SheetPeer, to: &mut SheetPeer) {
    for f in from.replica.drain_outbox() {
        for r in to.replica.receive(&f).unwrap() {
            from.replica.receive(&r).unwrap();
        }
    }
}

fn sheet_settle(a: &mut SheetPeer, b: &mut SheetPeer) {
    for _ in 0..4 {
        sheet_send(a, b);
        sheet_send(b, a);
        a.pull();
        b.pull();
    }
}

fn sheet_pair() -> (SheetPeer, SheetPeer) {
    let mk = |id: u64| {
        let mut engine = SheetEngine::with_bundled_fonts();
        engine.set_locale("en-US");
        engine.new_workbook("chung.xlsx").unwrap();
        engine.set_input(0, 0, 0, "Tên").unwrap();
        SheetPeer {
            engine,
            replica: Session::with_client_id(id),
        }
    };
    let (mut a, mut b) = (mk(1), mk(2));
    let c = a.engine.collab_start(true, 1).unwrap();
    a.replica.seed(&to_replica(&c)).unwrap();
    b.engine.collab_start(false, 2).unwrap();
    let s = a.replica.start().unwrap();
    for r in b.replica.receive(&s).unwrap() {
        a.replica.receive(&r).unwrap();
    }
    let s = b.replica.start().unwrap();
    for r in a.replica.receive(&s).unwrap() {
        b.replica.receive(&r).unwrap();
    }
    sheet_settle(&mut a, &mut b);
    let full = b.replica.full_state();
    b.engine.collab_apply(&to_engine(&full)).unwrap();
    (a, b)
}

/// A 2×1 PNG.
const PNG_2X1: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x7b, 0x40, 0xe8,
    0xdd, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0x00, 0x04,
    0xff, 0x01, 0x07, 0x00, 0x01, 0xff, 0xe2, 0x23, 0x9e, 0x59, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

#[test]
fn sheet_cell_pictures_and_checkboxes_converge() {
    let (mut a, mut b) = sheet_pair();
    a.exec(&json!({"cmd":"placeImageInCell","sheet":0,"row":2,"col":1,"data":b64(PNG_2X1),"ext":"png","alt":"Logo"}));
    b.exec(&json!({"cmd":"insertCheckbox","sheet":0,"range":{"r0":0,"c0":2,"r1":2,"c1":2}}));
    sheet_settle(&mut a, &mut b);
    let cell = |p: &SheetPeer, r: u32, c: u32| {
        p.engine.workbook().unwrap().sheets[0]
            .cells
            .get(&(r, c))
            .cloned()
    };
    let img = cell(&b, 2, 1)
        .and_then(|c| c.image)
        .expect("picture reached B");
    assert_eq!(img.alt_text.as_deref(), Some("Logo"));
    assert_eq!(cell(&a, 2, 1), cell(&b, 2, 1));
    for r in 0..3 {
        assert_eq!(cell(&a, r, 2), cell(&b, r, 2));
    }
    let wa = a.engine.workbook().unwrap();
    let xf = wa.styles.cell_xfs[cell(&a, 0, 2).unwrap().style as usize].clone();
    assert!(xf.checkbox, "checkbox format reached A");
}

#[test]
fn sheet_theme_persons_and_workbook_protection_converge() {
    let (mut a, mut b) = sheet_pair();
    // A theme applied by A: the theme part (added) and its colours.
    let r = a.exec(
        &json!({"cmd":"applyThemeFile","file":b64(&corpus("slide","slide-notes-basic.pptx"))}),
    );
    assert_eq!(r["ok"], true, "{r}");
    // A threaded comment by B creates a person.
    b.exec(&json!({"cmd":"addComment","sheet":0,"row":0,"col":0,"text":"Kiểm tra số liệu","author":"Bình"}));
    sheet_settle(&mut a, &mut b);
    let (wa, wb) = (a.engine.workbook().unwrap(), b.engine.workbook().unwrap());
    assert!(wb.theme.is_some());
    assert_eq!(wa.theme, wb.theme);
    assert_eq!(wa.opaque.parts, wb.opaque.parts);
    assert_eq!(wa.opaque.workbook_rels, wb.opaque.workbook_rels);
    assert!(wa.persons.iter().any(|p| p.display_name == "Bình"));
    assert_eq!(wa.persons, wb.persons);
    assert_eq!(
        wa.sheets[0].threaded_comments,
        wb.sheets[0].threaded_comments
    );
    // Workbook protection set by B reaches A.
    b.exec(&json!({"cmd":"protectWorkbook","password":""}));
    sheet_settle(&mut a, &mut b);
    let has = |p: &SheetPeer| {
        p.engine
            .workbook()
            .unwrap()
            .unknown
            .iter()
            .any(|x| x.xml.contains("workbookProtection"))
    };
    assert!(has(&a));
    assert!(has(&b));
    // Both saved files carry the theme.
    let sa = a.engine.save("xlsx", 0).unwrap();
    let sb = b.engine.save("xlsx", 0).unwrap();
    assert_eq!(
        bo_engine::variant::theme_of_package(&sa),
        bo_engine::variant::theme_of_package(&sb)
    );
}
