//! BlueSlide opens `.ppt` ([MS-PPT] shapes, pictures, text) and `.odp`
//! (ODF 1.3 drawing) decks and saves them as `.pptx`; BlueSheet / BlueSlide
//! encryption and macro reporting.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_engine::Engine;
use bo_model::slide::ShapeNode;
use std::path::PathBuf;

fn corpus(ext: &str) -> Vec<(String, Vec<u8>)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/corpus/slide");
    let mut v: Vec<(String, Vec<u8>)> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == ext))
        .map(|p| {
            (
                p.file_name().unwrap().to_string_lossy().into_owned(),
                std::fs::read(&p).unwrap(),
            )
        })
        .collect();
    v.sort();
    v
}

/// Shapes of every slide (groups flattened), with their text.
fn shapes(e: &Engine) -> Vec<Vec<(String, String)>> {
    let p = e.presentation().unwrap();
    p.slides
        .iter()
        .map(|s| {
            let mut out = Vec::new();
            bo_pptx::visit(&s.data.shapes, &mut |n| match n {
                ShapeNode::Shape(sh) => out.push((
                    "sp".to_owned(),
                    sh.text
                        .as_ref()
                        .map(bo_model::slide::TextBody::text)
                        .unwrap_or_default(),
                )),
                ShapeNode::Picture(_) => out.push(("pic".to_owned(), String::new())),
                _ => {}
            });
            out
        })
        .collect()
}

#[test]
fn ppt_decks_open_with_shapes_and_pictures_and_save_as_pptx() {
    let files = corpus("ppt");
    assert_eq!(files.len(), 3);
    for (name, bytes) in &files {
        let mut e = Engine::with_bundled_fonts();
        let info = e.open(bytes, name).unwrap();
        assert!(info.name.ends_with(".pptx"), "{name}: {}", info.name);
        assert_eq!(info.page_count, 2, "{name}");
        for i in 0..info.page_count {
            assert!(!e.slide(i).unwrap().commands.is_empty(), "{name} slide {i}");
        }
        let sh = shapes(&e);
        let out = e.save_pptx().unwrap();
        let back = bo_pptx::read_pptx(&out).unwrap();
        assert_eq!(back.slides.len(), 2);
        match name.as_str() {
            "binary-ppt-01-shapes.ppt" => {
                assert_eq!(sh[0].len(), 6, "{sh:?}");
                assert_eq!(sh[0][0].1, "Hình cơ bản");
                assert!(sh[0].iter().any(|(_, t)| t == "Elip"));
                assert!(e.slide_notes(0).unwrap().contains("Năm hình"));
            }
            "binary-ppt-02-pictures.ppt" => {
                assert_eq!(sh[0].iter().filter(|(k, _)| k == "pic").count(), 2);
                assert_eq!(sh[1].iter().filter(|(k, _)| k == "pic").count(), 1);
            }
            _ => {
                assert!(sh[0].iter().any(|(_, t)| t == "Chữ đậm 24pt màu xanh"));
                assert_eq!(sh[1].len(), 3);
            }
        }
    }
}

#[test]
fn odp_decks_open_with_drawing_and_save_as_pptx() {
    let files = corpus("odp");
    assert_eq!(files.len(), 3);
    for (name, bytes) in &files {
        let mut e = Engine::with_bundled_fonts();
        let info = e.open(bytes, name).unwrap();
        assert!(info.name.ends_with(".pptx"));
        assert!(info.page_count >= 1);
        // 25.4 cm × 19.05 cm page → 720 × 540 pt.
        assert_eq!(
            (info.pages[0].width, info.pages[0].height),
            (720.0, 540.0),
            "{name}"
        );
        let sh = shapes(&e);
        match name.as_str() {
            "odf-odp-01-shapes.odp" => {
                assert_eq!(sh[0].len(), 6, "{sh:?}");
                assert!(sh[0].iter().any(|(_, t)| t == "Đậm và nghiêng"));
                assert_eq!(sh[0].iter().filter(|(k, _)| k == "pic").count(), 1);
            }
            "odf-odp-02-text-styles.odp" => {
                assert_eq!(info.page_count, 2);
                assert!(e.slide_notes(0).unwrap().contains("Ghi chú trang chữ"));
            }
            _ => assert_eq!(sh[0].iter().filter(|(k, _)| k == "pic").count(), 2),
        }
        assert!(bo_pptx::read_pptx(&e.save_pptx().unwrap()).is_ok());
    }
}

#[test]
fn encrypted_deck_asks_for_the_password_and_keeps_it() {
    let (_, pptx) = &corpus("pptx")[0];
    let enc = bo_crypto::encrypt(pptx, "slide-pw").unwrap();
    let mut e = Engine::with_bundled_fonts();
    assert_eq!(
        e.open(&enc, "a.pptx").unwrap_err().to_string(),
        "password required"
    );
    assert_eq!(
        e.open_with_password(&enc, "a.pptx", Some("x"))
            .unwrap_err()
            .to_string(),
        "wrong password"
    );
    let info = e
        .open_with_password(&enc, "a.pptx", Some("slide-pw"))
        .unwrap();
    assert!(info.protected);
    let saved = e.save_pptx().unwrap();
    assert!(bo_crypto::is_encrypted(&saved));
    // Password removed through the slide command bridge.
    e.slide_command_json(r#"{"cmd":"setSavePassword"}"#)
        .unwrap();
    assert!(e.save_pptx().unwrap().starts_with(b"PK"));
    // A plain deck after that has no password.
    let info = e.open(pptx, "b.pptx").unwrap();
    assert!(!info.protected);
}
