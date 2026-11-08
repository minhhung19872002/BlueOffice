//! Live collaboration (Phase 6): two engines, each with its own `bo-collab`
//! replica, exchanging real y-sync frames. Concurrent edits must converge to
//! the same model on both sides.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

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
            self.engine.collab_start(seed, id).unwrap()
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

    /// Like [`Peer::cmd`] but a refused command is fine (nothing to share).
    fn try_cmd(&mut self, json: &str) {
        let r: serde_json::Value =
            serde_json::from_str(&self.engine.command_json(json).unwrap()).unwrap();
        if let Ok(c) = serde_json::from_value::<CollabChange>(r["collab"]["change"].clone()) {
            self.replica.apply_local(&to_replica(&c)).unwrap();
        }
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
fn table_cells_merge_cell_by_cell() {
    let (mut a, mut b) = pair("trước\nsau");
    a.cmd(&sel(0, "trước".len()));
    a.cmd(r#"{"cmd":"insertTable","rows":2,"cols":2}"#);
    settle(&mut a, &mut b);
    converged(&a, &b);
    // Paragraphs: trước, 4 cells, sau.
    assert_eq!(b.text().len(), 6, "{:?}", b.text());
    // Different cells and the same cell, concurrently.
    a.cmd(&sel(1, 0));
    a.cmd(&insert("An ô 1"));
    b.cmd(&sel(4, 0));
    b.cmd(&insert("Bình ô 4"));
    a.cmd(&sel(2, 0));
    a.cmd(&insert("A2"));
    b.cmd(&sel(2, 0));
    b.cmd(&insert("B2"));
    settle(&mut a, &mut b);
    converged(&a, &b);
    let t = a.text();
    assert_eq!(t[1], "An ô 1");
    assert_eq!(t[4], "Bình ô 4");
    assert!(t[2] == "A2B2" || t[2] == "B2A2", "{t:?}");
    assert_eq!(tables(&a), tables(&b));
    // Only the cells travel: the body text still holds one table line.
    assert_eq!(a.replica.body_text(), "trước\n\nsau\n");
    // Saving after remote edits gives a readable document.
    let bytes = b.engine.save_docx().unwrap();
    let mut c = Engine::with_bundled_fonts();
    c.open(&bytes, "x.docx").unwrap();
}

/// (rows, cells per row) of every top-level table.
fn tables(p: &Peer) -> Vec<Vec<usize>> {
    p.engine.document().unwrap().sections[0]
        .blocks
        .iter()
        .filter_map(|b| match b {
            bo_model::Block::Table(t) => Some(t.rows.iter().map(|r| r.cells.len()).collect()),
            _ => None,
        })
        .collect()
}

fn table_edit(kind: &str) -> String {
    format!(r#"{{"cmd":"tableEdit","edit":{{"kind":"{kind}"}}}}"#)
}

#[test]
fn table_rows_and_columns_converge() {
    let (mut a, mut b) = pair("đầu");
    a.cmd(&sel(0, "đầu".len()));
    a.cmd(r#"{"cmd":"insertTable","rows":2,"cols":2}"#);
    a.cmd(&sel(1, 0));
    a.cmd(&insert("x"));
    settle(&mut a, &mut b);
    assert_eq!(tables(&b), vec![vec![2, 2]]);
    // A inserts a row below the first, B a column right of the first and
    // types in the last cell — all at the same time.
    a.cmd(&sel(1, 0));
    a.cmd(r#"{"cmd":"tableEdit","edit":{"kind":"insertRow","below":true}}"#);
    b.cmd(&sel(1, 0));
    b.cmd(r#"{"cmd":"tableEdit","edit":{"kind":"insertColumn","right":true}}"#);
    let last = b.text().len() - 1;
    b.cmd(&sel(last, 0));
    b.cmd(&insert("cuối"));
    settle(&mut a, &mut b);
    converged(&a, &b);
    assert_eq!(tables(&a), tables(&b));
    let t = tables(&a);
    assert_eq!(t[0].len(), 3, "{t:?}");
    // A's new row also gets a cell in B's new column (it was short before).
    assert_eq!(t[0], vec![3, 3, 3], "{t:?}");
    assert!(a.text().iter().any(|p| p == "cuối"));
    assert!(a.text().iter().any(|p| p == "x"));
    // Concurrent delete of a row and an edit in another row.
    a.cmd(&sel(1, 0));
    a.cmd(&table_edit("deleteRow"));
    let last = b.text().len() - 1;
    b.cmd(&sel(last, 0));
    b.cmd(&insert("!"));
    settle(&mut a, &mut b);
    converged(&a, &b);
    assert_eq!(tables(&a), tables(&b));
    assert_eq!(tables(&a)[0].len(), 2);
    assert!(a.text().iter().any(|p| p == "!cuối"), "{:?}", a.text());
}

#[test]
fn styles_comments_notes_and_headers_converge() {
    let (mut a, mut b) = pair("một hai ba\nbốn năm");
    // Styles: a built-in style added by A reaches B as a definition.
    a.cmd(&sel(1, 0));
    a.cmd(r#"{"cmd":"applyStyle","id":"Heading2"}"#);
    settle(&mut a, &mut b);
    let style = |p: &Peer| {
        p.engine
            .document()
            .unwrap()
            .styles
            .styles
            .iter()
            .find(|s| s.id == "Heading2")
            .cloned()
    };
    assert!(style(&b).is_some());
    assert_eq!(style(&a), style(&b));
    // Comments: add, reply, resolve.
    a.cmd(&range(0, 0, 0, "một".len()));
    a.cmd(r#"{"cmd":"addComment","text":"Xem lại"}"#);
    settle(&mut a, &mut b);
    let comments = |p: &Peer| p.engine.document().unwrap().comments.clone();
    assert_eq!(comments(&b).len(), 1);
    let id = comments(&b)[0].id;
    b.cmd(&format!(
        r#"{{"cmd":"replyComment","id":{id},"text":"Đồng ý"}}"#
    ));
    settle(&mut a, &mut b);
    assert_eq!(comments(&a).len(), 2);
    a.cmd(&format!(r#"{{"cmd":"resolveComment","id":{id}}}"#));
    settle(&mut a, &mut b);
    converged(&a, &b);
    let texts = |p: &Peer| {
        comments(p)
            .iter()
            .map(|c| (c.text(), c.done, c.author.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(texts(&a), texts(&b));
    assert!(comments(&b)[0].done);
    assert_eq!(comments(&b)[1].text(), "Đồng ý");
    // Footnotes: A inserts one, B changes its text.
    a.cmd(&sel(1, "bốn".len()));
    a.cmd(r#"{"cmd":"insertNote","text":"Chú thích"}"#);
    settle(&mut a, &mut b);
    converged(&a, &b);
    let notes = |p: &Peer| {
        p.engine
            .document()
            .unwrap()
            .footnotes
            .iter()
            .map(|(id, b)| (*id, bo_model::write::blocks_word_count(b)))
            .collect::<Vec<_>>()
    };
    assert_eq!(notes(&a), notes(&b));
    let nid = *b
        .engine
        .document()
        .unwrap()
        .footnotes
        .keys()
        .find(|k| **k > 0)
        .unwrap();
    b.cmd(&format!(
        r#"{{"cmd":"setNoteText","id":{nid},"text":"Chú thích đã sửa"}}"#
    ));
    settle(&mut a, &mut b);
    assert_eq!(notes(&a), notes(&b));
    assert_eq!(
        a.engine.document().unwrap().footnotes[&nid],
        b.engine.document().unwrap().footnotes[&nid]
    );
    // Delete the comment thread.
    b.cmd(&format!(r#"{{"cmd":"deleteComment","id":{id}}}"#));
    settle(&mut a, &mut b);
    assert_eq!(comments(&a), Vec::new());
    converged(&a, &b);
}

/// A real file with merged cells: both users type at the end of two cells.
#[test]
fn merged_table_cells_of_a_real_file_merge() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/corpus/doc/tables-02-merge.docx");
    let bytes = std::fs::read(path).unwrap();
    let open = |id| {
        let mut engine = Engine::with_bundled_fonts();
        engine.open(&bytes, "gộp.docx").unwrap();
        Peer {
            engine,
            replica: Session::with_client_id(id),
            slides: false,
        }
    };
    let (mut a, mut b) = (open(91), open(92));
    a.start(true);
    b.start(false);
    connect(&mut a, &mut b);
    settle(&mut a, &mut b);
    converged(&a, &b);
    let t = a.text();
    let pa = t.iter().position(|p| p.starts_with("Gộp ngang")).unwrap();
    let pb = t.iter().position(|p| p.starts_with("Gộp dọc")).unwrap();
    a.cmd(&sel(pa, t[pa].len()));
    b.cmd(&sel(pb, t[pb].len()));
    // Typed key by key while the other's keys arrive: carets stay in their cells.
    let (ka, kb): (Vec<char>, Vec<char>) =
        (" của An".chars().collect(), " của Bình".chars().collect());
    for i in 0..ka.len().max(kb.len()) {
        if let Some(ch) = ka.get(i) {
            a.cmd(&insert(&ch.to_string()));
        }
        if let Some(ch) = kb.get(i) {
            b.cmd(&insert(&ch.to_string()));
        }
        send(&mut a, &mut b);
        b.pull();
        send(&mut b, &mut a);
        a.pull();
    }
    settle(&mut a, &mut b);
    converged(&a, &b);
    let t = b.text();
    assert!(t.iter().any(|p| p == "Gộp ngang của An"), "{t:?}");
    assert!(t.iter().any(|p| p == "Gộp dọc của Bình"), "{t:?}");
}

#[test]
fn header_text_is_shared() {
    let (mut a, mut b) = pair("thân");
    a.cmd(r#"{"cmd":"editHeaderFooter","page":0,"header":true}"#);
    a.cmd(&insert("Đầu trang"));
    a.cmd(r#"{"cmd":"closeHeaderFooter"}"#);
    settle(&mut a, &mut b);
    let header = |p: &Peer| {
        let s = &p.engine.document().unwrap().sections[0];
        s.headers.default.as_ref().map(|b| {
            b.iter()
                .filter_map(|x| x.as_paragraph())
                .map(bo_model::Paragraph::plain_text)
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(header(&a), Some(vec!["Đầu trang".to_owned()]));
    assert_eq!(header(&a), header(&b));
    // Both edit the header text concurrently: the edits merge.
    a.cmd(r#"{"cmd":"editHeaderFooter","page":0,"header":true}"#);
    a.cmd(&insert(" A"));
    a.cmd(r#"{"cmd":"closeHeaderFooter"}"#);
    b.cmd(&sel(0, 0));
    b.cmd(&insert("B "));
    settle(&mut a, &mut b);
    converged(&a, &b);
    assert_eq!(header(&a), header(&b));
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
    big_stack(slide_move_and_text_edit_converge_inner);
}

fn slide_move_and_text_edit_converge_inner() {
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
    /// Table structure at the caret's paragraph (ignored outside a table).
    Table(usize, u8),
}

fn edit() -> impl Strategy<Value = Edit> {
    let n = 0usize..40;
    prop_oneof![
        (n.clone(), n.clone(), "[a-zâ ]{1,4}").prop_map(|(p, o, t)| Edit::Type(p, o, t)),
        (n.clone(), n.clone(), 1usize..5).prop_map(|(p, o, l)| Edit::Delete(p, o, l)),
        (n.clone(), n.clone()).prop_map(|(p, o)| Edit::Split(p, o)),
        n.clone().prop_map(Edit::Merge),
        (n.clone(), n.clone(), 1usize..6).prop_map(|(p, o, l)| Edit::Bold(p, o, l)),
        (n.clone(), 0u8..4).prop_map(|(p, k)| Edit::Table(p, k)),
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
        Edit::Table(p, k) => {
            let (p, _) = caret(peer, *p, 0);
            peer.cmd(&sel(p, 0));
            let edit = [
                r#"{"kind":"insertRow","below":true}"#,
                r#"{"kind":"insertColumn"}"#,
                r#"{"kind":"deleteRow"}"#,
                r#"{"kind":"deleteColumn"}"#,
            ][usize::from(*k)];
            peer.try_cmd(&format!(r#"{{"cmd":"tableEdit","edit":{edit}}}"#));
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
        // A table (shared cell by cell) between the paragraphs.
        a.cmd(&sel(0, 3));
        a.cmd(r#"{"cmd":"insertTable","rows":2,"cols":2}"#);
        settle(&mut a, &mut b);
        for (ea, eb) in &rounds {
            for e in ea { apply(&mut a, e); }
            for e in eb { apply(&mut b, e); }
            settle(&mut a, &mut b);
            prop_assert_eq!(a.engine.collab_runs(), b.engine.collab_runs());
            prop_assert_eq!(a.model(), b.model());
            prop_assert_eq!(tables(&a), tables(&b));
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

/// Runs `f` with a large stack: debug builds keep a slide master's
/// text styles on the stack while (de)serializing them.
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

fn deck_pair(a: &str, b: &str) -> (Peer, Peer) {
    let mut a = Peer::deck(1, a);
    let mut b = Peer::deck(2, b);
    a.start(true);
    b.start(false);
    connect(&mut a, &mut b);
    settle(&mut a, &mut b);
    (a, b)
}

fn slide_ids(p: &Peer) -> Vec<u32> {
    p.engine
        .presentation()
        .unwrap()
        .slides
        .iter()
        .map(|s| s.slide_id)
        .collect()
}

#[test]
fn concurrent_slide_moves_keep_both_moves() {
    big_stack(|| {
        let (mut a, mut b) = deck_pair("slide-edit-slides.pptx", "slide-edit-slides.pptx");
        let ids = slide_ids(&a);
        assert!(ids.len() >= 3, "{ids:?}");
        let n = ids.len();
        // A moves the first slide to the end, B the last slide to the front.
        a.slide(&format!(r#"{{"cmd":"moveSlide","from":0,"to":{}}}"#, n - 1));
        b.slide(&format!(r#"{{"cmd":"moveSlide","from":{},"to":0}}"#, n - 1));
        settle(&mut a, &mut b);
        assert_eq!(slide_ids(&a), slide_ids(&b));
        let got = slide_ids(&a);
        assert_eq!(got[0], ids[n - 1], "{ids:?} → {got:?}");
        assert_eq!(got[n - 1], ids[0], "{ids:?} → {got:?}");
        assert_eq!(
            a.engine.presentation().unwrap(),
            b.engine.presentation().unwrap()
        );
    });
}

#[test]
fn notes_text_merges_and_parts_follow_the_room() {
    big_stack(|| {
        // A joiner opens the same file version (packages parts are not shared).
        let (mut a, mut b) = deck_pair("slide-notes-basic.pptx", "slide-notes-basic.pptx");
        assert_eq!(
            a.engine.presentation().unwrap(),
            b.engine.presentation().unwrap()
        );
        let notes = |p: &Peer| {
            p.engine.presentation().unwrap().slides[0]
                .notes
                .as_ref()
                .map(bo_model::slide::Notes::text)
                .unwrap_or_default()
        };
        a.slide(r#"{"cmd":"setNotes","text":"Ghi chú chung"}"#);
        settle(&mut a, &mut b);
        assert_eq!(notes(&b), "Ghi chú chung");
        // Both type in the notes at once: both edits survive.
        a.slide(r#"{"cmd":"setNotes","text":"An: Ghi chú chung"}"#);
        b.slide(r#"{"cmd":"setNotes","text":"Ghi chú chung — Bình"}"#);
        settle(&mut a, &mut b);
        assert_eq!(notes(&a), notes(&b));
        assert_eq!(notes(&a), "An: Ghi chú chung — Bình");
        assert_eq!(
            a.engine.presentation().unwrap(),
            b.engine.presentation().unwrap()
        );
        // A master changed elsewhere (last writer wins per part) reaches B.
        let mut m = a.engine.presentation().unwrap().masters[0].clone();
        m.master_id += 7;
        let mut c = Change::default();
        c.meta
            .insert("sm:0".into(), Some(serde_json::to_string(&m).unwrap()));
        a.replica.apply_local(&c).unwrap();
        settle(&mut a, &mut b);
        assert_eq!(
            b.engine.presentation().unwrap().masters[0].master_id,
            m.master_id
        );
    });
}

// source: ECMA-376 Part 1, §17.13.4.2 (comment ids), §17.11.10 (footnote ids)
#[test]
fn concurrent_comments_and_notes_get_distinct_ids() {
    let (mut a, mut b) = pair("một hai ba\nbốn năm");
    // Both add a comment and a footnote before seeing each other's.
    a.cmd(&range(0, 0, 0, "một".len()));
    a.cmd(r#"{"cmd":"addComment","text":"Của A"}"#);
    b.cmd(&range(1, 0, 1, "bốn".len()));
    b.cmd(r#"{"cmd":"addComment","text":"Của B"}"#);
    a.cmd(&sel(0, "một hai".len()));
    a.cmd(r#"{"cmd":"insertNote","text":"Chú thích A"}"#);
    b.cmd(&sel(1, "bốn năm".len()));
    b.cmd(r#"{"cmd":"insertNote","text":"Chú thích B"}"#);
    settle(&mut a, &mut b);
    converged(&a, &b);
    for p in [&a, &b] {
        let doc = p.engine.document().unwrap();
        let mut texts: Vec<(i32, String)> = doc.comments.iter().map(|c| (c.id, c.text())).collect();
        texts.sort_by(|x, y| x.1.cmp(&y.1));
        assert_eq!(texts.len(), 2, "{texts:?}");
        assert_ne!(texts[0].0, texts[1].0);
        assert_eq!(
            (texts[0].1.as_str(), texts[1].1.as_str()),
            ("Của A", "Của B")
        );
        let notes: Vec<i32> = doc.footnotes.keys().copied().filter(|&k| k > 1).collect();
        assert_eq!(notes.len(), 2, "{notes:?}");
    }
}

// source: ECMA-376 Part 1, §17.4.17 gridSpan (rows shorter than the grid)
#[test]
fn a_cell_deleted_on_purpose_stays_deleted() {
    let (mut a, mut b) = pair("đầu");
    a.cmd(&sel(0, "đầu".len()));
    a.cmd(r#"{"cmd":"insertTable","rows":2,"cols":3}"#);
    settle(&mut a, &mut b);
    // A deletes the first cell of the first row (shift left): a short row.
    a.cmd(&sel(1, 0));
    a.cmd(&table_edit("deleteCell"));
    settle(&mut a, &mut b);
    converged(&a, &b);
    assert_eq!(tables(&b), vec![vec![2, 3]]);
    // B then inserts a column: the short row gets the new cell, not the deleted one.
    b.cmd(&sel(1, 0));
    b.cmd(r#"{"cmd":"tableEdit","edit":{"kind":"insertColumn","right":true}}"#);
    settle(&mut a, &mut b);
    converged(&a, &b);
    assert_eq!(tables(&a), tables(&b));
    let t = tables(&a);
    assert_eq!(t[0][1], 4, "{t:?}");
    assert_eq!(t[0][0], 3, "{t:?}");
}

/// Paragraph ids (`p` of a position) of the cells of tables nested in cells.
fn nested_paras(p: &Peer) -> Vec<usize> {
    body_paragraphs(p.engine.document().unwrap())
        .iter()
        .enumerate()
        .filter(|(_, path)| path.len() > 4)
        .map(|(i, _)| i)
        .collect()
}

// source: ECMA-376 Part 1, §17.4.38 tbl (a table in a table cell, §17.4.66 tc)
#[test]
fn edits_in_different_cells_of_a_nested_table_merge() {
    let (mut a, mut b) = pair("đầu");
    a.cmd(&sel(0, "đầu".len()));
    a.cmd(r#"{"cmd":"insertTable","rows":2,"cols":2}"#);
    // A table inside the first cell.
    a.cmd(&sel(1, 0));
    a.cmd(r#"{"cmd":"insertTable","rows":2,"cols":2}"#);
    settle(&mut a, &mut b);
    converged(&a, &b);
    let (na, nb) = (nested_paras(&a), nested_paras(&b));
    assert_eq!(na, nb);
    assert!(na.len() >= 4, "{na:?}");
    // A types in the first nested cell, B in the last one, at the same time.
    a.cmd(&sel(na[0], 0));
    a.cmd(&insert("của A"));
    b.cmd(&sel(nb[3], 0));
    b.cmd(&insert("của B"));
    settle(&mut a, &mut b);
    converged(&a, &b);
    let text = a.text();
    assert!(text.iter().any(|p| p == "của A"), "{text:?}");
    assert!(text.iter().any(|p| p == "của B"), "{text:?}");
    // Still one nested table of 2×2 in the first cell.
    let doc = a.engine.document().unwrap();
    let bo_model::Block::Table(outer) = doc.sections[0]
        .blocks
        .iter()
        .find(|b| matches!(b, bo_model::Block::Table(_)))
        .unwrap()
    else {
        unreachable!()
    };
    let inner: Vec<_> = outer.rows[0].cells[0]
        .blocks
        .iter()
        .filter_map(|b| match b {
            bo_model::Block::Table(t) => {
                Some(t.rows.iter().map(|r| r.cells.len()).collect::<Vec<_>>())
            }
            _ => None,
        })
        .collect();
    assert_eq!(inner, vec![vec![2, 2]]);
}

/// A peer whose engine copied slide `index` of `source` and then opened `target`.
fn deck_with_clip(id: u64, source: &str, index: usize, target: &[u8]) -> (Peer, String) {
    let mut p = Peer::deck(id, source);
    let text = bo_engine::slide_edit::clip::plain_text(
        &p.engine.presentation().unwrap().slides[index].data.shapes,
    );
    p.engine
        .slide_command_json(&format!(r#"{{"cmd":"copySlides","indices":[{index}]}}"#))
        .unwrap();
    p.engine.open(target, "chung.pptx").unwrap();
    (p, text)
}

// source: ECMA-376 Part 1, §19.3.1.39 sldLayout, §19.3.1.42 sldMaster, §20.1.6.9 theme
#[test]
fn designs_imported_at_once_by_two_peers_are_both_kept() {
    big_stack(|| {
        // A deck whose layouts fit no pasted slide: pasting imports layout + master + theme.
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/slide/slide-shapes-presets.pptx");
        let mut target = bo_pptx::read_pptx(&std::fs::read(p).unwrap()).unwrap();
        for l in &mut target.layouts {
            l.data.name = Some("Riêng".into());
            l.data.shapes.clear();
        }
        let bare = bo_pptx::write_pptx(&target).unwrap();
        let (mut a, ta) = deck_with_clip(1, "slide-text-bullets.pptx", 1, &bare);
        let (mut b, tb) = deck_with_clip(2, "slide-notes-basic.pptx", 0, &bare);
        a.start(true);
        b.start(false);
        connect(&mut a, &mut b);
        settle(&mut a, &mut b);
        let (l0, m0, t0) = {
            let p = a.engine.presentation().unwrap();
            (p.layouts.len(), p.masters.len(), p.themes.len())
        };
        // Both paste at once: each imports a design at the same index.
        a.slide(r#"{"cmd":"paste"}"#);
        b.slide(r#"{"cmd":"paste"}"#);
        settle(&mut a, &mut b);
        let pa = a.engine.presentation().unwrap();
        assert_eq!(pa, b.engine.presentation().unwrap());
        assert_eq!(
            (pa.layouts.len(), pa.masters.len(), pa.themes.len()),
            (l0 + 2, m0 + 2, t0 + 2)
        );
        let layout_of = |text: &str| {
            let s = pa
                .slides
                .iter()
                .find(|s| bo_engine::slide_edit::clip::plain_text(&s.data.shapes) == text)
                .unwrap_or_else(|| panic!("pasted slide {text:?}"));
            s.layout
        };
        let (la, lb) = (layout_of(&ta), layout_of(&tb));
        assert!(la >= l0 && lb >= l0 && la != lb, "{la} {lb}");
        let (ma, mb) = (pa.layouts[la].master, pa.layouts[lb].master);
        assert!(ma >= m0 && mb >= m0 && ma != mb, "{ma} {mb}");
        let (tha, thb) = (pa.masters[ma].theme, pa.masters[mb].theme);
        assert!(tha >= t0 && thb >= t0 && tha != thb, "{tha} {thb}");
        // Each pasted slide still has the design it came with.
        let src = |name: &str, i: usize| {
            let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/corpus/slide")
                .join(name);
            let d = bo_pptx::read_pptx(&std::fs::read(p).unwrap()).unwrap();
            let l = &d.layouts[d.slides[i].layout];
            (
                l.data.name.clone(),
                d.themes[d.masters[l.master].theme].name.clone(),
            )
        };
        assert_eq!(
            src("slide-text-bullets.pptx", 1),
            (
                pa.layouts[la].data.name.clone(),
                pa.themes[tha].name.clone()
            )
        );
        assert_eq!(
            src("slide-notes-basic.pptx", 0),
            (
                pa.layouts[lb].data.name.clone(),
                pa.themes[thb].name.clone()
            )
        );
        // The saved file is valid on both sides.
        let bytes = a.engine.save_pptx().unwrap();
        let back = bo_pptx::read_pptx(&bytes).unwrap();
        assert_eq!(back.layouts.len(), l0 + 2);
    });
}
