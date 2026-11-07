//! Two (or three) replicas talking y-sync through a dumb relay converge.
use bo_collab::{Presence, Session};
use serde_json::{json, Map, Value};

fn no_attrs() -> Map<String, Value> {
    Map::new()
}

fn attrs(v: &Value) -> Map<String, Value> {
    v.as_object().cloned().unwrap()
}

/// Full handshake: each side's start message is answered by the other.
fn connect(a: &mut Session, b: &mut Session) {
    let from_a = a.start().unwrap();
    let from_b = b.start().unwrap();
    for r in b.receive(&from_a).unwrap() {
        a.receive(&r).unwrap();
    }
    for r in a.receive(&from_b).unwrap() {
        b.receive(&r).unwrap();
    }
}

/// Relay every queued broadcast of `from` to all `to`.
fn flush(from: &mut Session, to: &mut [&mut Session]) {
    for msg in from.drain_outbox() {
        for t in to.iter_mut() {
            t.receive(&msg).unwrap();
        }
    }
}

#[test]
fn initial_sync_transfers_existing_state() {
    let mut a = Session::with_client_id(1);
    a.insert_text(0, "Cộng hoà xã hội chủ nghĩa Việt Nam\n", &no_attrs())
        .unwrap();
    a.set_cell("Sheet1", "A1", "=SUM(B1:B3)").unwrap();
    a.drain_outbox();
    let mut b = Session::with_client_id(2);
    connect(&mut a, &mut b);
    assert_eq!(b.body_text(), "Cộng hoà xã hội chủ nghĩa Việt Nam\n");
    assert_eq!(b.cells()["Sheet1!A1"], "=SUM(B1:B3)");
}

#[test]
fn concurrent_edits_converge() {
    let mut a = Session::with_client_id(1);
    let mut b = Session::with_client_id(2);
    let mut c = Session::with_client_id(3);
    connect(&mut a, &mut b);
    connect(&mut a, &mut c);
    connect(&mut b, &mut c);
    a.insert_text(0, "Hello world", &no_attrs()).unwrap();
    flush(&mut a, &mut [&mut b, &mut c]);

    // Concurrent: A bolds "Hello", B inserts at the end, C deletes "world".
    a.format_text(0, 5, &attrs(&json!({"bold": true}))).unwrap();
    b.insert_text(11, "!", &no_attrs()).unwrap();
    c.delete_text(6, 5).unwrap();
    a.set_cell("S", "B2", "1").unwrap();
    b.set_cell("S", "B2", "2").unwrap();
    flush(&mut a, &mut [&mut b, &mut c]);
    flush(&mut b, &mut [&mut a, &mut c]);
    flush(&mut c, &mut [&mut a, &mut b]);

    assert_eq!(a.body_text(), "Hello !");
    assert_eq!(a.body_text(), b.body_text());
    assert_eq!(b.body_text(), c.body_text());
    assert_eq!(a.body_delta(), c.body_delta());
    assert_eq!(a.body_delta()[0].insert, "Hello");
    assert_eq!(a.body_delta()[0].attributes["bold"], json!(true));
    assert_eq!(a.cells(), b.cells());

    // A server snapshot restores the same state into a fresh replica.
    let mut fresh = Session::with_client_id(9);
    fresh.load_snapshot(&c.snapshot()).unwrap();
    assert_eq!(fresh.body_delta(), a.body_delta());
    assert_eq!(fresh.cells(), a.cells());
}

#[test]
fn undo_only_reverts_local_changes() {
    let mut a = Session::with_client_id(1);
    let mut b = Session::with_client_id(2);
    connect(&mut a, &mut b);
    a.insert_text(0, "abc", &no_attrs()).unwrap();
    flush(&mut a, &mut [&mut b]);
    b.insert_text(3, "XYZ", &no_attrs()).unwrap();
    flush(&mut b, &mut [&mut a]);
    assert_eq!(a.body_text(), "abcXYZ");

    assert!(a.undo());
    flush(&mut a, &mut [&mut b]);
    assert_eq!(a.body_text(), "XYZ");
    assert_eq!(b.body_text(), "XYZ");
    assert!(!a.undo(), "remote edits are not on A's undo stack");
    assert!(a.redo());
    flush(&mut a, &mut [&mut b]);
    assert_eq!(b.body_text(), "abcXYZ");
}

#[test]
fn shapes_and_presence() {
    let mut a = Session::with_client_id(1);
    let mut b = Session::with_client_id(2);
    connect(&mut a, &mut b);
    a.set_shape("s1", "title", &json!({"x": 10, "text": "Tiêu đề"}))
        .unwrap();
    a.set_presence(&Presence {
        user_id: "u1".into(),
        name: "Trần Thị Bình".into(),
        color: "#2b6cb0".into(),
        selection: Some((0, 3)),
        cell: None,
        shape: Some("s1/title".into()),
    })
    .unwrap();
    flush(&mut a, &mut [&mut b]);
    assert_eq!(b.shapes()["s1/title"]["text"], json!("Tiêu đề"));
    let peers = b.peers();
    assert_eq!(peers.len(), 1);
    assert_eq!(peers[0].0, 1);
    assert_eq!(peers[0].1.name, "Trần Thị Bình");
    b.remove_shape("s1", "title");
    flush(&mut b, &mut [&mut a]);
    assert!(a.shapes().is_empty());
}

#[test]
fn rejects_bad_input_without_panicking() {
    let mut a = Session::with_client_id(1);
    assert!(a.delete_text(0, 1).is_err());
    assert!(a.insert_text(5, "x", &no_attrs()).is_err());
    assert!(a.set_cell("", "A1", "1").is_err());
    assert!(a.set_shape("a/b", "c", &json!({})).is_err());
    assert!(a.receive(&[0xff, 0xff, 0xff]).is_err() || a.body_text().is_empty());
    assert_eq!(a.receive(&[]).unwrap().len(), 0);
}
