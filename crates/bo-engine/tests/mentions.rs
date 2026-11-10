//! @mentions and assigned tasks in comments (CORE-COMMENT-001): BlueDoc
//! (`mailto:` hyperlink + [MS-DOCX] people part + BlueOffice comment tasks part),
//! BlueSheet ([MS-XLSX] `mentions` + persons, task in the comment's `extLst`) and
//! BlueSlide (text + BlueOffice `extLst` extension). Saving, reopening and saving
//! again keeps every mention unchanged.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::{Engine, SheetEngine};
use serde_json::{json, Value};

fn cmd(e: &mut Engine, v: &Value) -> Value {
    serde_json::from_str(&e.command_json(&v.to_string()).unwrap()).unwrap()
}

fn part(bytes: &[u8], name: &str) -> Option<String> {
    let pkg = bo_opc::Package::read(bytes, &bo_opc::Limits::default()).unwrap();
    pkg.part(name)
        .map(|p| String::from_utf8_lossy(&p.data).into_owned())
}

/// Kept in the temp directory for OpenXmlValidator (`bo-cli validate`).
fn keep(name: &str, bytes: &[u8]) {
    let dir = std::env::temp_dir().join("bo-mentions");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(name), bytes).unwrap();
}

fn binh() -> Value {
    json!({"id": "u2", "name": "Trần Thị Bình", "email": "binh@drive.invalid"})
}

/// "Xem lại @Trần Thị Bình nhé": the mention is UTF-16 [8, 22).
fn mention_text() -> (String, Value) {
    let text = "Xem lại @Trần Thị Bình nhé".to_owned();
    let start = "Xem lại ".encode_utf16().count();
    let len = "@Trần Thị Bình".encode_utf16().count();
    let m = json!([{"id": "u2", "name": "Trần Thị Bình", "email": "binh@drive.invalid",
                    "start": start, "length": len}]);
    (text, m)
}

#[test]
fn doc_mentions_people_part_and_task_round_trip() {
    let mut e = Engine::with_bundled_fonts();
    e.open("Quyết định số 12".as_bytes(), "a.txt").unwrap();
    cmd(
        &mut e,
        &json!({"cmd": "setAuthor", "name": "Nguyễn Văn An"}),
    );
    let (text, mentions) = mention_text();
    let r = cmd(
        &mut e,
        &json!({"cmd": "addComment", "text": text, "mentions": mentions,
                "task": {"assignee": binh()}}),
    );
    let c = &r["review"]["comments"][0];
    assert_eq!(c["text"], text);
    assert_eq!(c["mentions"][0]["name"], "Trần Thị Bình");
    assert_eq!(c["mentions"][0]["email"], "binh@drive.invalid");
    assert_eq!(c["mentions"][0]["start"], 8);
    assert_eq!(c["task"]["assignee"]["name"], "Trần Thị Bình");

    let saved = e.save_docx().unwrap();
    // The mention is a mailto: hyperlink of the comments part, the person is in
    // the people part, the task in the BlueOffice part.
    let rels = part(&saved, "/word/_rels/comments.xml.rels").unwrap();
    assert!(rels.contains("mailto:binh@drive.invalid"), "{rels}");
    assert!(rels.contains("TargetMode=\"External\""));
    let main_rels = part(&saved, "/word/_rels/document.xml.rels").unwrap();
    assert!(!main_rels.contains("mailto:"), "{main_rels}");
    let people = part(&saved, "/word/people.xml").unwrap();
    assert!(people.contains("w15:author=\"Trần Thị Bình\""), "{people}");
    assert!(people.contains("w15:userId=\"binh@drive.invalid\""));
    let tasks = part(&saved, "/word/blueoffice/commentTasks.xml").unwrap();
    assert!(tasks.contains("assigneeId=\"u2\""), "{tasks}");

    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "a.docx").unwrap();
    let back = cmd(&mut f, &json!({"cmd": "review"}));
    let b = &back["review"]["comments"][0];
    assert_eq!(
        b["mentions"],
        c["mentions"]
            .clone()
            .as_array()
            .map(|a| {
                // Read back from the file: the host id is not stored in the hyperlink.
                let mut m = a[0].clone();
                m["id"] = json!("");
                json!([m])
            })
            .unwrap()
    );
    assert_eq!(b["task"], c["task"]);
    // Unchanged: saving again keeps the parts byte for byte.
    let again = f.save_docx().unwrap();
    for p in [
        "/word/comments.xml",
        "/word/_rels/comments.xml.rels",
        "/word/people.xml",
        "/word/blueoffice/commentTasks.xml",
    ] {
        assert_eq!(part(&again, p), part(&saved, p), "{p}");
    }

    // Another comment (with another mention): the first mention, its person and
    // its task stay; resolving completes the task.
    cmd(
        &mut f,
        &json!({"cmd": "setSelection", "selection": {"anchor": {"p": 0, "o": 0}, "focus": {"p": 0, "o": 3}}}),
    );
    cmd(
        &mut f,
        &json!({"cmd": "addComment", "text": "@Lê Văn Cường xem",
                "mentions": [{"id": "u3", "name": "Lê Văn Cường", "email": "cuong@drive.invalid", "start": 0, "length": 13}]}),
    );
    let id = b["id"].as_i64().unwrap();
    cmd(&mut f, &json!({"cmd": "resolveComment", "id": id}));
    let edited = f.save_docx().unwrap();
    keep("mentions.docx", &edited);
    let people = part(&edited, "/word/people.xml").unwrap();
    assert!(people.contains("Trần Thị Bình") && people.contains("Lê Văn Cường"));
    let mut g = Engine::with_bundled_fonts();
    g.open(&edited, "a.docx").unwrap();
    let list = cmd(&mut g, &json!({"cmd": "review"}))["review"]["comments"].clone();
    let first = list
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == id)
        .unwrap();
    assert_eq!(first["mentions"], b["mentions"]);
    assert_eq!(first["task"], b["task"]);
    assert_eq!(first["done"], true);
    assert!(list
        .as_array()
        .unwrap()
        .iter()
        .any(|c| c["mentions"][0]["email"] == "cuong@drive.invalid"));
}

#[test]
fn doc_people_part_of_the_source_is_kept() {
    // A file whose people part already lists the person (as Word writes it):
    // the original entry (provider AD) is kept, nothing is duplicated.
    let mut e = Engine::with_bundled_fonts();
    e.open("Văn bản".as_bytes(), "a.txt").unwrap();
    let (text, mentions) = mention_text();
    cmd(
        &mut e,
        &json!({"cmd": "addComment", "text": text, "mentions": mentions}),
    );
    let saved = e.save_docx().unwrap();
    let mut pkg = bo_opc::Package::read(&saved, &bo_opc::Limits::default()).unwrap();
    let ad = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<w15:people xmlns:w15=\"http://schemas.microsoft.com/office/word/2012/wordml\"><w15:person w15:author=\"Trần Thị Bình\"><w15:presenceInfo w15:providerId=\"AD\" w15:userId=\"S::binh@corp::1\"/></w15:person></w15:people>";
    pkg.put_part("/word/people.xml", ad.as_bytes().to_vec());
    let word = pkg.write().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&word, "a.docx").unwrap();
    cmd(&mut f, &json!({"cmd": "addComment", "text": "Thêm"}));
    let out = f.save_docx().unwrap();
    assert_eq!(part(&out, "/word/people.xml").as_deref(), Some(ad));
}

fn exec(s: &mut SheetEngine, v: &Value) -> Value {
    serde_json::from_str(&s.exec_json(&v.to_string()).unwrap()).unwrap()
}

#[test]
fn sheet_mentions_persons_and_task_round_trip() {
    let mut s = SheetEngine::with_bundled_fonts();
    s.open(b"a,b\n1,2\n", "t.csv").unwrap();
    let (text, mentions) = mention_text();
    exec(
        &mut s,
        &json!({"cmd": "addComment", "sheet": 0, "row": 1, "col": 1, "text": text,
                "author": "Nguyễn Văn An", "date": "2026-11-10T08:00:00.000Z",
                "mentions": mentions, "task": {"assignee": binh()}}),
    );
    let t = exec(&mut s, &json!({"cmd": "comments", "sheet": 0}));
    let c = &t[0]["comments"][0];
    assert_eq!(c["mentions"][0]["name"], "Trần Thị Bình");
    assert_eq!(c["mentions"][0]["email"], "binh@drive.invalid");
    assert_eq!(c["mentions"][0]["start"], 8);
    assert_eq!(c["task"]["assignee"]["id"], "u2");

    let saved = s.save("xlsx", 0).unwrap();
    keep("mentions.xlsx", &saved);
    let tc = part(&saved, "/xl/threadedComments/threadedComment1.xml").unwrap();
    assert!(
        tc.contains("<mentions><mention mentionpersonId=\"{"),
        "{tc}"
    );
    assert!(tc.contains("startIndex=\"8\" length=\"14\""), "{tc}");
    assert!(tc.contains(bo_model::mention::EXT_URI));
    let wb = bo_xlsx::read_xlsx(&saved).unwrap();
    let m = &wb.sheets[0].threaded_comments[0].mentions[0];
    let p = wb.persons.iter().find(|p| p.id == m.person_id).unwrap();
    assert_eq!(p.display_name, "Trần Thị Bình");
    assert_eq!(p.user_id.as_deref(), Some("binh@drive.invalid"));

    let mut r = SheetEngine::with_bundled_fonts();
    r.open(&saved, "t.xlsx").unwrap();
    assert_eq!(exec(&mut r, &json!({"cmd": "comments", "sheet": 0})), t);
    // Saving again keeps the mention ids and the parts unchanged.
    let again = r.save("xlsx", 0).unwrap();
    for p in [
        "/xl/threadedComments/threadedComment1.xml",
        "/xl/persons/person.xml",
    ] {
        assert_eq!(part(&again, p), part(&saved, p), "{p}");
    }
}

fn slide(e: &mut Engine, v: &Value) -> Value {
    serde_json::from_str(&e.slide_command_json(&v.to_string()).unwrap()).unwrap()
}

#[test]
fn slide_mentions_and_task_round_trip() {
    let mut e = Engine::with_bundled_fonts();
    e.open(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/corpus/slide/slide-notes-basic.pptx"),
        )
        .unwrap(),
        "a.pptx",
    )
    .unwrap();
    let (text, mentions) = mention_text();
    let r = slide(
        &mut e,
        &json!({"cmd": "addComment", "text": text, "author": "Nguyễn Văn An",
                "date": "2026-11-10T08:00:00.000Z", "mentions": mentions,
                "task": {"assignee": binh()}}),
    );
    let c = r["comments"][0]["comments"][0].clone();
    assert_eq!(c["text"], text);
    assert_eq!(c["mentions"][0]["email"], "binh@drive.invalid");
    assert_eq!(c["task"]["assignee"]["name"], "Trần Thị Bình");
    let saved = e.save_pptx().unwrap();
    keep("mentions.pptx", &saved);
    let mut f = Engine::with_bundled_fonts();
    f.open(&saved, "a.pptx").unwrap();
    let back = slide(&mut f, &json!({"cmd": "state"}));
    assert_eq!(back["comments"][0]["comments"][0], c);
    // A reply rewrites the part: the first comment keeps its mention and task.
    let id = back["comments"][0]["id"].as_str().unwrap().to_owned();
    let r = slide(
        &mut f,
        &json!({"cmd": "addComment", "text": "Đã xem", "author": "Trần Thị Bình", "parent": id}),
    );
    assert_eq!(r["comments"][0]["comments"][0], c);
}
