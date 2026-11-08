//! Threaded comments in BlueSheet ([MS-XLSX] threaded comments / person parts)
//! and modern comments in BlueSlide ([MS-PPTX] p188:cmLst): read, reply,
//! resolve, delete, undo and a save round trip.

use bo_engine::SheetEngine;
use serde_json::{json, Value};

fn corpus(path: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus")
            .join(path),
    )
    .unwrap()
}

fn exec(s: &mut SheetEngine, v: &Value) -> Value {
    serde_json::from_str(&s.exec_json(&v.to_string()).unwrap()).unwrap()
}

#[test]
fn sheet_threads_reply_resolve_delete_and_save() {
    let mut s = SheetEngine::with_bundled_fonts();
    s.open(&corpus("sheet/threaded-2.xlsx"), "t.xlsx").unwrap();
    let threads = exec(&mut s, &json!({"cmd": "comments", "sheet": 0}));
    assert_eq!(threads.as_array().unwrap().len(), 2);
    assert_eq!(threads[0]["cell"], "B2");
    assert_eq!(threads[0]["comments"].as_array().unwrap().len(), 3);
    assert_eq!(threads[0]["comments"][0]["author"], "Nguyễn Văn An");
    let root = threads[0]["id"].as_str().unwrap().to_owned();

    // Reply as a new person, a new thread on C3, resolve the first.
    exec(
        &mut s,
        &json!({"cmd": "addComment", "sheet": 0, "text": "Đồng ý", "author": "Lê Cường",
                "date": "2026-11-08T10:00:00.000Z", "parent": root}),
    );
    let new = exec(
        &mut s,
        &json!({"cmd": "addComment", "sheet": 0, "row": 2, "col": 2, "text": "Ô mới", "author": "Lê Cường"}),
    );
    let new_id = new["id"].as_str().unwrap().to_owned();
    assert!(new_id.starts_with('{') && new_id.len() == 38, "{new_id}");
    exec(
        &mut s,
        &json!({"cmd": "resolveComment", "sheet": 0, "id": root, "done": true}),
    );
    let t = exec(&mut s, &json!({"cmd": "comments", "sheet": 0}));
    assert_eq!(t.as_array().unwrap().len(), 3);
    assert_eq!(t[0]["done"], true);
    assert_eq!(t[0]["comments"][3]["text"], "Đồng ý");
    assert_eq!(t[0]["comments"][3]["author"], "Lê Cường");
    assert_eq!(t[2]["cell"], "C3");
    // A second thread on the same cell is refused.
    assert!(s
        .exec_json(
            &json!({"cmd": "addComment", "sheet": 0, "row": 2, "col": 2, "text": "x"}).to_string()
        )
        .is_err());

    // Saved and reopened: the threads, the new person and the legacy note survive.
    let saved = s.save("xlsx", 0).unwrap();
    std::fs::create_dir_all(std::env::temp_dir().join("bo-comments")).unwrap();
    std::fs::write(
        std::env::temp_dir().join("bo-comments/sheet-threads.xlsx"),
        &saved,
    )
    .unwrap();
    let mut r = SheetEngine::with_bundled_fonts();
    r.open(&saved, "t.xlsx").unwrap();
    assert_eq!(exec(&mut r, &json!({"cmd": "comments", "sheet": 0})), t);
    let wb = bo_xlsx::read_xlsx(&saved).unwrap();
    assert!(wb.persons.iter().any(|p| p.display_name == "Lê Cường"));
    assert!(wb.sheets[0]
        .comments
        .iter()
        .any(|c| c.author == format!("tc={new_id}")));

    // Delete the new thread, then undo the delete and the resolve.
    exec(
        &mut s,
        &json!({"cmd": "deleteComment", "sheet": 0, "id": new_id}),
    );
    assert_eq!(
        exec(&mut s, &json!({"cmd": "comments", "sheet": 0}))
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(s.undo().unwrap());
    assert!(s.undo().unwrap());
    let t = exec(&mut s, &json!({"cmd": "comments", "sheet": 0}));
    assert_eq!(t.as_array().unwrap().len(), 3);
    assert_eq!(t[0]["done"], false);
}

fn slide(e: &mut bo_engine::Engine, v: &Value) -> Value {
    serde_json::from_str(&e.slide_command_json(&v.to_string()).unwrap()).unwrap()
}

#[test]
fn slide_threads_reply_resolve_delete_and_save() {
    let mut e = bo_engine::Engine::with_bundled_fonts();
    e.open(&corpus("slide/slide-notes-basic.pptx"), "a.pptx")
        .unwrap();
    let r = slide(&mut e, &json!({"cmd": "state"}));
    assert_eq!(r["comments"], json!([]));
    let r = slide(
        &mut e,
        &json!({"cmd": "addComment", "text": "Đổi màu tiêu đề", "author": "Nguyễn Văn An",
                "date": "2026-11-08T09:00:00.000Z"}),
    );
    assert!(r.get("error").is_none(), "{r}");
    let id = r["comments"][0]["id"].as_str().unwrap().to_owned();
    let r = slide(
        &mut e,
        &json!({"cmd": "addComment", "text": "Đã đổi\nsang xanh", "author": "Trần Bình",
                "date": "2026-11-08T09:05:00.000Z", "parent": id}),
    );
    let r2 = slide(
        &mut e,
        &json!({"cmd": "resolveComment", "id": id, "done": true}),
    );
    assert_eq!(r2["comments"][0]["done"], true);
    let c = &r["comments"][0]["comments"];
    assert_eq!(c[0]["author"], "Nguyễn Văn An");
    assert_eq!(c[1]["text"], "Đã đổi\nsang xanh");
    assert_eq!(c[1]["author"], "Trần Bình");
    // Slide 2 has its own (empty) list.
    let r = slide(&mut e, &json!({"cmd": "goTo", "slide": 1}));
    assert_eq!(r["comments"], json!([]));
    slide(&mut e, &json!({"cmd": "goTo", "slide": 0}));

    // Saved and reopened: the comment and authors parts round-trip.
    let saved = e.save_pptx().unwrap();
    std::fs::create_dir_all(std::env::temp_dir().join("bo-comments")).unwrap();
    std::fs::write(
        std::env::temp_dir().join("bo-comments/slide-threads.pptx"),
        &saved,
    )
    .unwrap();
    let mut f = bo_engine::Engine::with_bundled_fonts();
    f.open(&saved, "a.pptx").unwrap();
    let back = slide(&mut f, &json!({"cmd": "state"}));
    assert_eq!(back["comments"], r2["comments"]);

    // Delete the reply, then the thread; undo brings both back.
    let reply = r2["comments"][0]["comments"][1]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    slide(&mut e, &json!({"cmd": "deleteComment", "id": reply}));
    let r = slide(&mut e, &json!({"cmd": "deleteComment", "id": id}));
    assert_eq!(r["comments"], json!([]));
    slide(&mut e, &json!({"cmd": "undo"}));
    let r = slide(&mut e, &json!({"cmd": "undo"}));
    assert_eq!(r["comments"], r2["comments"]);
    let r = slide(&mut e, &json!({"cmd": "addComment", "text": "  "}));
    assert!(r.get("error").is_some());
}
