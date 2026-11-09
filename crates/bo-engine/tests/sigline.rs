//! Signature lines over the JSON bridge (CORE-SIGLINE-001): insert two lines,
//! save, sign the first and then the second with `bo-sign` (each signature names
//! its line, [MS-OFFCRYPTO] SignatureInfoV1/SetupID), reopen and see the signed
//! state on the right line, then edit and see every signature break.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use base64::Engine as _;
use bo_engine::Engine;
use bo_render::Command;
use bo_sign::ooxml::{verify_ooxml, SignatureLineSigning};
use bo_sign::pdf::SignOptions;
use bo_sign::report::verify_document;
use bo_sign::trust::TrustStore;
use serde_json::Value;
use std::time::{Duration, UNIX_EPOCH};

const ID1: &str = "{6F1B2C3D-4E5F-4061-8273-94A5B6C7D8E9}";
/// Signing time (Unix seconds).
const T0: u64 = 1_793_000_000;

fn cmd(e: &mut Engine, json: &str) -> Value {
    let r: Value = serde_json::from_str(&e.command_json(json).unwrap()).unwrap();
    assert!(r.get("error").is_none(), "{json}: {:?}", r.get("error"));
    r
}

fn b64(s: &Value) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(s.as_str().unwrap())
        .unwrap()
}

fn with_two_lines() -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.new_document("hop-dong.docx").unwrap();
    cmd(&mut e, r#"{"cmd":"insertText","text":"Bên A"}"#);
    cmd(&mut e, r#"{"cmd":"splitParagraph"}"#);
    cmd(
        &mut e,
        &format!(
            r#"{{"cmd":"insertSignatureLine","id":"{ID1}","spec":{{"suggestedSigner":"Nguyễn Văn A","suggestedSigner2":"Giám đốc","suggestedSignerEmail":"a@example.vn","signingInstructions":"Ký và ghi rõ họ tên","allowComments":true,"showSignDate":true}}}}"#
        ),
    );
    cmd(&mut e, r#"{"cmd":"splitParagraph"}"#);
    cmd(&mut e, r#"{"cmd":"insertText","text":"Bên B"}"#);
    cmd(&mut e, r#"{"cmd":"splitParagraph"}"#);
    let r = cmd(
        &mut e,
        r#"{"cmd":"insertSignatureLine","spec":{"suggestedSigner":"Trần Thị B","showSignDate":false}}"#,
    );
    let lines = r["signatureLines"].as_array().unwrap();
    assert_eq!(lines.len(), 2);
    e
}

fn line_ids(e: &mut Engine) -> Vec<String> {
    let r = cmd(e, r#"{"cmd":"signatureLines"}"#);
    r["signatureLines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["line"]["id"].as_str().unwrap().to_owned())
        .collect()
}

fn signer(cn: &str) -> bo_sign::SoftwareSigner {
    bo_sign::testing::self_signed(&mut rand::thread_rng(), cn)
}

/// Signs line `id` of `bytes` the way the editor does: the line pictures come
/// from the engine that shows the document.
fn sign_line(e: &mut Engine, bytes: &[u8], id: &str, name: &str, cn: &str) -> Vec<u8> {
    let r = cmd(
        e,
        &format!(
            r#"{{"cmd":"signatureLineImages","id":"{id}","text":"{name}","date":"2026-11-09"}}"#
        ),
    );
    let imgs = &r["signatureLineImages"];
    let (valid, invalid) = (b64(&imgs["valid"]), b64(&imgs["invalid"]));
    // PNG signature (ISO/IEC 15948 §5.2).
    assert!(valid.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_ne!(valid, invalid, "the invalid picture is marked");
    let opts = SignOptions {
        time: UNIX_EPOCH + Duration::from_secs(T0),
        signature_line: Some(SignatureLineSigning {
            setup_id: id.into(),
            text: name.into(),
            image: Vec::new(),
            comments: String::new(),
            valid_image: valid,
            invalid_image: invalid,
        }),
        ..SignOptions::default()
    };
    bo_sign::sign_document(bytes, &signer(cn), &opts, None).unwrap()
}

fn signed_texts(e: &mut Engine) -> Vec<Option<String>> {
    let r = cmd(e, r#"{"cmd":"signatureLines"}"#);
    r["signatureLines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["line"]["signed"]["text"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn inserted_lines_are_drawn_saved_and_reopened() {
    let mut e = with_two_lines();
    let ids = line_ids(&mut e);
    assert_eq!(ids[0], ID1);
    // The second id was derived: a braced GUID.
    assert_eq!(ids[1].len(), 38);
    assert!(ids[1].starts_with('{') && ids[1].ends_with('}'));
    // Each line is drawn as a shape (cross, rule, signer captions).
    let page = e.page(0).unwrap();
    let paths = page
        .commands
        .iter()
        .filter(|c| matches!(c, Command::Path { .. }))
        .count();
    assert!(paths >= 4, "{paths} paths");
    let bytes = e.save_docx().unwrap();
    let mut f = Engine::with_bundled_fonts();
    f.open(&bytes, "hop-dong.docx").unwrap();
    assert_eq!(line_ids(&mut f), ids);
    let r = cmd(&mut f, r#"{"cmd":"signatureLines"}"#);
    let first = &r["signatureLines"][0]["line"];
    assert_eq!(first["suggestedSigner2"], "Giám đốc");
    assert_eq!(first["signingInstructions"], "Ký và ghi rõ họ tên");
    assert_eq!(first["allowComments"], true);
    assert_eq!(r["signatureLines"][1]["line"]["showSignDate"], false);
    // One undo removes the last line.
    cmd(&mut e, r#"{"cmd":"undo"}"#);
    assert_eq!(line_ids(&mut e).len(), 1);
}

#[test]
fn signing_both_lines_in_turn_then_editing() {
    let mut e = with_two_lines();
    let ids = line_ids(&mut e);
    let saved = e.save_docx().unwrap();

    // Sign line 1; the reopened document shows it signed, line 2 still open.
    let once = sign_line(&mut e, &saved, &ids[0], "Nguyễn Văn A", "Nguyễn Văn A");
    let mut f = Engine::with_bundled_fonts();
    f.open(&once, "hop-dong.docx").unwrap();
    assert_eq!(
        signed_texts(&mut f),
        vec![Some("Nguyễn Văn A".to_owned()), None]
    );
    let sigs = verify_ooxml(&once).unwrap();
    assert_eq!(
        sigs[0].signature_line.as_ref().unwrap().setup_id,
        ids[0].as_str()
    );

    // Sign line 2 on the signed file itself (unchanged): both verify.
    let twice = sign_line(&mut f, &once, &ids[1], "Trần Thị B", "Trần Thị B");
    let now = UNIX_EPOCH + Duration::from_secs(T0 + 100);
    let report = verify_document(&twice, &TrustStore::new(), now).unwrap();
    assert!(report.error.is_none(), "{:?}", report.error);
    let lines: Vec<(String, bool)> = report
        .signature_lines
        .iter()
        .map(|l| (l.id.clone(), l.valid))
        .collect();
    assert_eq!(lines, vec![(ids[0].clone(), true), (ids[1].clone(), true)]);
    let mut g = Engine::with_bundled_fonts();
    g.open(&twice, "hop-dong.docx").unwrap();
    // BO_SIGLINE_OUT=<dir>: keep the files to look at / validate them.
    if let Some(dir) = std::env::var_os("BO_SIGLINE_OUT") {
        let dir = std::path::Path::new(&dir);
        std::fs::write(dir.join("sigline-unsigned.docx"), &saved).unwrap();
        std::fs::write(dir.join("sigline-signed.docx"), &twice).unwrap();
        std::fs::write(
            dir.join("sigline-signed.png"),
            g.render_png(0, 1.5).unwrap(),
        )
        .unwrap();
    }
    assert_eq!(
        signed_texts(&mut g),
        vec![
            Some("Nguyễn Văn A".to_owned()),
            Some("Trần Thị B".to_owned())
        ]
    );
    // The host shows the results on the lines (display only: not a modification).
    let r = cmd(
        &mut g,
        &format!(
            r#"{{"cmd":"setSignatureLineStatus","lines":[{{"id":"{}","valid":true}},{{"id":"{}","valid":false}}]}}"#,
            ids[0], ids[1]
        ),
    );
    assert_eq!(r["modified"], false);
    assert_eq!(r["signatureLines"][1]["line"]["signed"]["valid"], false);

    // Editing the signed document breaks every signature.
    cmd(&mut g, r#"{"cmd":"insertText","text":"sửa sau khi ký "}"#);
    let edited = g.save_docx().unwrap();
    assert!(verify_ooxml(&edited).is_err());
    let report = verify_document(&edited, &TrustStore::new(), now).unwrap();
    assert!(report.error.is_some());
    assert_eq!(report.signature_lines.len(), 2);
    assert!(report.signature_lines.iter().all(|l| !l.valid));
}
