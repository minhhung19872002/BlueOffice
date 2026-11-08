//! Live collaboration (Phase 6): two engines, each with its own `bo-collab`
//! replica, exchanging real y-sync frames. Concurrent edits must converge to
//! the same model on both sides.

use bo_collab::{Change, Session};
use bo_engine::slide_edit::geom::place_slide;
use bo_engine::{CollabChange, Engine};
use bo_model::index::{body_paragraphs, para_at};
use bo_model::slide::ShapeNode;
use proptest::prelude::*;
use std::path::PathBuf;

/// Paragraph text, runs and paragraph properties.
type ParaView = (
    String,
    Vec<(String, bo_model::RunProps)>,
    bo_model::ParaProps,
);

struct Peer {
    engine: Engine,
    replica: Session,
    slides: bool,
}

fn to_replica(c: &CollabChange) -> Change {
    serde_json::from_value(serde_json::to_value(c).unwrap()).unwrap()
}

fn to_engine(c: &Change) -> CollabChange {
    serde_json::from_value(serde_json::to_value(c).unwrap()).unwrap()
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
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/slide")
            .join(name);
        let mut engine = Engine::with_bundled_fonts();
        engine.open(&std::fs::read(p).unwrap(), name).unwrap();
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
            self.engine.collab_start(seed).unwrap()
        };
        if seed {
            self.replica.seed(&to_replica(&c)).unwrap();
        }
    }

    /// Runs a BlueDoc command and shares its change.
    fn cmd(&mut self, json: &str) {
        let r: serde_json::Value =
            serde_json::from_str(&self.engine.command_json(json).unwrap()).unwrap();
        assert!(r.get("error").is_none(), "{json}: {r}");
        let c: CollabChange = serde_json::from_value(r["collab"]["change"].clone()).unwrap();
        self.replica.apply_local(&to_replica(&c)).unwrap();
    }

    fn slide(&mut self, json: &str) {
        let r: serde_json::Value =
            serde_json::from_str(&self.engine.slide_command_json(json).unwrap()).unwrap();
        assert!(r.get("error").is_none(), "{json}: {r}");
        let c: CollabChange = serde_json::from_value(r["collab"]["change"].clone()).unwrap();
        self.replica.apply_local(&to_replica(&c)).unwrap();
    }

    /// Applies what the replica received to the engine.
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
        self.replica.apply_local(&to_replica(&back)).unwrap();
    }

    /// Paragraph texts and run formatting of the model.
    fn model(&self) -> Vec<ParaView> {
        let doc = self.engine.document().unwrap();
        body_paragraphs(doc)
            .iter()
            .filter_map(|p| para_at(doc, p))
            .map(|p| {
                let mut runs: Vec<(String, bo_model::RunProps)> = Vec::new();
                for r in &p.runs {
                    let t = r.plain_text();
                    if t.is_empty() {
                        continue;
                    }
                    match runs.last_mut() {
                        Some((pt, pp)) if *pp == r.props => pt.push_str(&t),
                        _ => runs.push((t, r.props.clone())),
                    }
                }
                (p.plain_text(), runs, p.props.clone())
            })
            .collect()
    }

    fn text(&self) -> Vec<String> {
        self.model().into_iter().map(|p| p.0).collect()
    }
}

/// Delivers `from`'s queued frames to `to` (replies go back).
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

/// Full exchange until both sides are quiet.
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

fn pair(text: &str) -> (Peer, Peer) {
    let mut a = Peer::doc(1, text);
    let mut b = Peer::doc(2, "khác hẳn\nnội dung");
    a.start(true);
    b.start(false);
    connect(&mut a, &mut b);
    settle(&mut a, &mut b);
    (a, b)
}

fn sel(p: usize, o: usize) -> String {
    format!(
        r#"{{"cmd":"setSelection","selection":{{"anchor":{{"p":{p},"o":{o}}},"focus":{{"p":{p},"o":{o}}}}}}}"#
    )
}

fn range(p: usize, a: usize, q: usize, b: usize) -> String {
    format!(
        r#"{{"cmd":"setSelection","selection":{{"anchor":{{"p":{p},"o":{a}}},"focus":{{"p":{q},"o":{b}}}}}}}"#
    )
}

fn insert(t: &str) -> String {
    format!(
        r#"{{"cmd":"insertText","text":{}}}"#,
        serde_json::to_string(t).unwrap()
    )
}

fn converged(a: &Peer, b: &Peer) {
    assert_eq!(a.engine.collab_runs(), b.engine.collab_runs());
    assert_eq!(a.model(), b.model());
}

#[test]
fn late_joiner_gets_the_seeded_body() {
    let (a, b) = pair("Cộng hoà xã hội\nchủ nghĩa Việt Nam");
    assert_eq!(b.text(), vec!["Cộng hoà xã hội", "chủ nghĩa Việt Nam"]);
    converged(&a, &b);
    let shared: String = a.replica.body_text();
    assert_eq!(shared, "Cộng hoà xã hội\nchủ nghĩa Việt Nam\n");
}

#[test]
fn concurrent_typing_in_one_paragraph_converges() {
    let (mut a, mut b) = pair("Hello world");
    a.cmd(&sel(0, 5));
    a.cmd(&insert(" big"));
    b.cmd(&sel(0, 11));
    b.cmd(&insert("!"));
    b.cmd(&sel(0, 0));
    b.cmd(&insert("Ồ, "));
    settle(&mut a, &mut b);
    converged(&a, &b);
    assert_eq!(a.text(), vec!["Ồ, Hello big world!"]);
    // B's caret stayed after what B typed although A inserted text before it.
    let r: serde_json::Value =
        serde_json::from_str(&b.engine.command_json(r#"{"cmd":"state"}"#).unwrap()).unwrap();
    assert_eq!(r["selection"]["focus"]["o"], "Ồ, ".len());
}

#[test]
fn formatting_and_typing_converge() {
    let (mut a, mut b) = pair("đậm nghiêng thường");
    a.cmd(&range(0, 0, 0, "đậm".len()));
    a.cmd(r#"{"cmd":"formatText","format":{"kind":"bold"}}"#);
    a.cmd(r#"{"cmd":"formatText","format":{"kind":"size","half_points":32}}"#);
    a.cmd(r#"{"cmd":"formatParagraph","format":{"kind":"align","align":"Center"}}"#);
    let n = "đậm ".len();
    b.cmd(&range(0, n, 0, n + "nghiêng".len()));
    b.cmd(r#"{"cmd":"formatText","format":{"kind":"italic"}}"#);
    b.cmd(r#"{"cmd":"formatText","format":{"kind":"color","hex":"C00000"}}"#);
    b.cmd(&sel(0, "đậm nghiêng thường".len()));
    b.cmd(&insert(" nữa"));
    settle(&mut a, &mut b);
    converged(&a, &b);
    let m = b.model();
    assert_eq!(m[0].0, "đậm nghiêng thường nữa");
    let runs = &m[0].1;
    assert_eq!(runs[0].0, "đậm");
    assert_eq!(runs[0].1.bold, Some(true));
    assert_eq!(runs[0].1.size.map(|s| s.0), Some(32));
    let it = runs.iter().find(|r| r.0 == "nghiêng").unwrap();
    assert_eq!(it.1.italic, Some(true));
    assert_eq!(
        it.1.color.map(bo_model::Color::to_hex),
        Some("C00000".into())
    );
    assert_eq!(m[0].2.align, Some(bo_model::Alignment::Center));
}

#[test]
fn split_and_merge_paragraphs_converge() {
    let (mut a, mut b) = pair("một hai\nba bốn");
    // A splits the first paragraph, B merges the two paragraphs.
    a.cmd(&sel(0, "một".len()));
    a.cmd(r#"{"cmd":"splitParagraph"}"#);
    b.cmd(&sel(1, 0));
    b.cmd(r#"{"cmd":"deleteBackward"}"#);
    settle(&mut a, &mut b);
    converged(&a, &b);
    assert_eq!(a.text(), vec!["một", " haiba bốn"]);
    // Lists and styles travel with the paragraph mark.
    a.cmd(&sel(1, 0));
    a.cmd(r#"{"cmd":"formatParagraph","format":{"kind":"bullets"}}"#);
    b.cmd(&sel(0, 0));
    b.cmd(r#"{"cmd":"applyStyle","id":"Heading1"}"#);
    settle(&mut a, &mut b);
    converged(&a, &b);
    let m = b.model();
    assert_eq!(m[0].2.style.as_deref(), Some("Heading1"));
    assert!(m[1].2.num.is_some());
    assert!(b
        .engine
        .document()
        .unwrap()
        .styles
        .styles
        .iter()
        .any(|s| s.id == "Heading1"));
}

/// A late joiner whose editor opened the file before the room's state arrived
/// types at its caret right away; a later edit of the other user keeps it.
#[test]
fn late_joiner_typing_at_its_first_caret_is_shared() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/doc/phase1-sample.docx");
    let bytes = std::fs::read(path).unwrap();
    let open = |id| {
        let mut engine = Engine::with_bundled_fonts();
        engine.open(&bytes, "mẫu.docx").unwrap();
        Peer {
            engine,
            replica: Session::with_client_id(id),
            slides: false,
        }
    };
    let (mut a, mut b) = (open(71), open(72));
    a.start(true);
    b.start(false);
    connect(&mut a, &mut b);
    settle(&mut a, &mut b);
    converged(&a, &b);
    b.cmd(&insert("Bình gõ ngay "));
    settle(&mut a, &mut b);
    // The joiner's caret stayed where its editor showed it (start of the
    // document), not pushed to the end by the shared text replacing its copy.
    for p in [&a, &b] {
        assert!(p.text()[0].starts_with("Bình gõ ngay "), "{:?}", p.text());
    }
    a.cmd(&sel(0, 0));
    a.cmd(&insert("An "));
    settle(&mut a, &mut b);
    converged(&a, &b);
    for p in [&a, &b] {
        let all = p.text().join("\n");
        assert!(all.contains("Bình gõ ngay"), "{all}");
        assert!(all.starts_with("An Bình gõ ngay "), "{all}");
    }
}

#[test]
fn tables_are_shared_as_one_object() {
    let (mut a, mut b) = pair("trước\nsau");
    a.cmd(&sel(0, "trước".len()));
    a.cmd(r#"{"cmd":"insertTable","rows":2,"cols":2}"#);
    b.cmd(&sel(1, 0));
    b.cmd(&insert("x"));
    settle(&mut a, &mut b);
    converged(&a, &b);
    let doc = b.engine.document().unwrap();
    assert!(doc.sections[0]
        .blocks
        .iter()
        .any(|bl| matches!(bl, bo_model::Block::Table(_))));
    // Saving after remote edits gives a readable document.
    let bytes = b.engine.save_docx().unwrap();
    let mut c = Engine::with_bundled_fonts();
    c.open(&bytes, "x.docx").unwrap();
}

#[test]
fn collaborative_undo_only_reverts_own_edits() {
    let (mut a, mut b) = pair("abc");
    a.cmd(&sel(0, 3));
    a.cmd(&insert("A"));
    settle(&mut a, &mut b);
    b.replica.stop_capturing();
    b.cmd(&sel(0, 0));
    b.cmd(&insert("B"));
    settle(&mut a, &mut b);
    assert_eq!(a.text(), vec!["BabcA"]);
    // A undoes through the replica: only A's insertion goes away.
    assert!(a.replica.undo());
    a.pull();
    settle(&mut a, &mut b);
    converged(&a, &b);
    assert_eq!(b.text(), vec!["Babc"]);
}

#[test]
fn peer_positions_map_to_carets() {
    let (mut a, mut b) = pair("một\nhai ba");
    let r: serde_json::Value =
        serde_json::from_str(&b.engine.command_json(&sel(1, "hai".len())).unwrap()).unwrap();
    let off = r["collab"]["selection"].clone();
    assert_eq!(off, serde_json::json!([7, 7]));
    let peers = a.engine.collab_peers(&[(7, 7), (0, 3)]).unwrap();
    assert!(peers[0].caret.is_some());
    assert_eq!(peers[0].rects.len(), 0);
    assert_eq!(peers[1].rects.len(), 1);
    let _ = a.engine.collab_peers_json("[[0,99]]").unwrap();
    a.cmd(&sel(0, 0));
}

#[test]
fn slide_move_and_text_edit_converge() {
    let mut a = Peer::deck(1, "slide-shapes-presets.pptx");
    let mut b = Peer::deck(2, "slide-shapes-presets.pptx");
    a.start(true);
    b.start(false);
    connect(&mut a, &mut b);
    settle(&mut a, &mut b);
    let pa = a.engine.presentation().unwrap().clone();
    assert_eq!(&pa, b.engine.presentation().unwrap());
    // A moves one shape, B edits the text of another, concurrently.
    let placed = place_slide(&pa, 0);
    let ids: Vec<u32> = placed
        .iter()
        .filter(|p| matches!(pa.slides[0].data.shapes[p.path[0]], ShapeNode::Shape(_)))
        .map(|p| p.id)
        .collect();
    let (mover, writer) = (ids[0], ids[1]);
    a.slide(&format!(r#"{{"cmd":"select","ids":[{mover}]}}"#));
    a.slide(r#"{"cmd":"transform","transform":{"kind":"move","dx":40,"dy":10},"snap":false}"#);
    b.slide(&format!(
        r#"{{"cmd":"setText","id":{writer},"text":"Xin chào"}}"#
    ));
    settle(&mut a, &mut b);
    let (x, y) = (
        a.engine.presentation().unwrap(),
        b.engine.presentation().unwrap(),
    );
    assert_eq!(x, y);
    let find = |id: u32| {
        let pl = place_slide(x, 0);
        pl.into_iter().find(|p| p.id == id).unwrap().b
    };
    let before = placed.iter().find(|p| p.id == mover).unwrap().b;
    assert!((find(mover).cx - before.cx - 40.0).abs() < 0.5);
    let text =
        match &x.slides[0].data.shapes[placed.iter().find(|p| p.id == writer).unwrap().path[0]] {
            ShapeNode::Shape(s) => s
                .text
                .as_ref()
                .map(bo_model::slide::TextBody::text)
                .unwrap_or_default(),
            _ => String::new(),
        };
    assert!(text.contains("Xin chào"), "{text}");
    // Slides added on both sides and a reorder converge too.
    a.slide(r#"{"cmd":"addSlide"}"#);
    b.slide(r#"{"cmd":"moveSlide","from":0,"to":1}"#);
    settle(&mut a, &mut b);
    assert_eq!(
        a.engine.presentation().unwrap(),
        b.engine.presentation().unwrap()
    );
    assert_eq!(a.engine.slide_count(), pa.slides.len() + 1);
    // Presence: the selected object's box.
    let key = format!("{}/{mover}", pa.slides[0].slide_id);
    let boxes = b.engine.slide_collab_peers(&[key, "x/1".into()]).unwrap();
    assert!(boxes[0].is_some());
    assert!(boxes[1].is_none());
}

#[test]
fn shapes_round_trip_json() {
    let a = Peer::deck(1, "slide-shapes-presets.pptx");
    for (i, n) in a.engine.presentation().unwrap().slides[0]
        .data
        .shapes
        .iter()
        .enumerate()
    {
        let j = serde_json::to_string(n).unwrap();
        match serde_json::from_str::<ShapeNode>(&j) {
            Ok(m) => assert_eq!(&m, n, "{i}"),
            Err(e) => panic!("{i}: {e} {}", &j[..j.len().min(300)]),
        }
    }
}

// ------------------------------------------------------------- property

#[derive(Debug, Clone)]
enum Edit {
    Type(usize, usize, String),
    Delete(usize, usize, usize),
    Split(usize, usize),
    Merge(usize),
    Bold(usize, usize, usize),
}

fn edit() -> impl Strategy<Value = Edit> {
    let n = 0usize..40;
    prop_oneof![
        (n.clone(), n.clone(), "[a-zâ ]{1,4}").prop_map(|(p, o, t)| Edit::Type(p, o, t)),
        (n.clone(), n.clone(), 1usize..5).prop_map(|(p, o, l)| Edit::Delete(p, o, l)),
        (n.clone(), n.clone()).prop_map(|(p, o)| Edit::Split(p, o)),
        n.clone().prop_map(Edit::Merge),
        (n.clone(), n.clone(), 1usize..6).prop_map(|(p, o, l)| Edit::Bold(p, o, l)),
    ]
}

/// A valid caret near (p, o) on this peer.
fn caret(peer: &Peer, p: usize, o: usize) -> (usize, usize) {
    let t = peer.text();
    let p = p % t.len();
    let s = &t[p];
    let chars: Vec<usize> = s.char_indices().map(|(i, _)| i).chain([s.len()]).collect();
    (p, chars[o % chars.len()])
}

fn apply(peer: &mut Peer, e: &Edit) {
    match e {
        Edit::Type(p, o, t) => {
            let (p, o) = caret(peer, *p, *o);
            peer.cmd(&sel(p, o));
            peer.cmd(&insert(t));
        }
        Edit::Delete(p, o, l) | Edit::Bold(p, o, l) => {
            let (p, o) = caret(peer, *p, *o);
            let s = peer.text()[p].clone();
            let end = s
                .char_indices()
                .map(|(i, _)| i)
                .chain([s.len()])
                .filter(|&i| i >= o)
                .nth(*l)
                .unwrap_or(s.len());
            peer.cmd(&range(p, o, p, end));
            if matches!(e, Edit::Bold(..)) {
                peer.cmd(r#"{"cmd":"formatText","format":{"kind":"bold"}}"#);
            } else {
                peer.cmd(r#"{"cmd":"deleteForward"}"#);
            }
        }
        Edit::Split(p, o) => {
            let (p, o) = caret(peer, *p, *o);
            peer.cmd(&sel(p, o));
            peer.cmd(r#"{"cmd":"splitParagraph"}"#);
        }
        Edit::Merge(p) => {
            let (p, _) = caret(peer, *p, 0);
            peer.cmd(&sel(p, 0));
            peer.cmd(r#"{"cmd":"deleteBackward"}"#);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]

    #[test]
    fn random_concurrent_edits_converge(
        rounds in prop::collection::vec(
            (prop::collection::vec(edit(), 0..4), prop::collection::vec(edit(), 0..4)),
            1..4,
        )
    ) {
        let (mut a, mut b) = pair("một hai ba\nbốn năm");
        for (ea, eb) in &rounds {
            for e in ea { apply(&mut a, e); }
            for e in eb { apply(&mut b, e); }
            settle(&mut a, &mut b);
            prop_assert_eq!(a.engine.collab_runs(), b.engine.collab_runs());
            prop_assert_eq!(a.model(), b.model());
            // The shadow is the shared text.
            let shared: String = a.engine.collab_runs().iter().map(|r| r.0.as_str()).collect();
            prop_assert_eq!(shared.replace('\u{FFFC}', ""), a.replica.body_text());
        }
    }
}

/// Desktop E2E shape: the joiner types at every kind of position of a real
/// file (tabs, manual breaks, fields, tables) after the seeder's first edit.
#[test]
fn joiner_typing_anywhere_in_a_real_file_converges() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/doc/phase1-sample.docx");
    let bytes = std::fs::read(path).unwrap();
    let open = |id| {
        let mut engine = Engine::with_bundled_fonts();
        engine.open(&bytes, "mẫu.docx").unwrap();
        Peer {
            engine,
            replica: Session::with_client_id(id),
            slides: false,
        }
    };
    let probe = open(1);
    let paras = probe.text();
    let mut failures = Vec::new();
    for (p, t) in paras.iter().enumerate() {
        let mid = (0..=t.len() / 2)
            .rev()
            .find(|&k| t.is_char_boundary(k))
            .unwrap_or(0);
        for o in [0, mid, t.len()] {
            let (mut a, mut b) = (open(81), open(82));
            a.start(true);
            b.start(false);
            connect(&mut a, &mut b);
            settle(&mut a, &mut b);
            a.cmd(&sel(0, 0));
            a.cmd(&insert("Web "));
            settle(&mut a, &mut b);
            let r: serde_json::Value =
                serde_json::from_str(&b.engine.command_json(&sel(p, o)).unwrap()).unwrap();
            if r.get("error").is_some() {
                continue;
            }
            let c: CollabChange = serde_json::from_value(r["collab"]["change"].clone()).unwrap();
            b.replica.apply_local(&to_replica(&c)).unwrap();
            b.cmd(&insert("ZZ"));
            settle(&mut a, &mut b);
            let (ta, tb) = (a.text().join("\n"), b.text().join("\n"));
            if ta != tb || !ta.contains("ZZ") || a.model() != b.model() {
                failures.push(format!("p={p} o={o} {t:?}\nA: {ta:?}\nB: {tb:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n---\n"));
}
