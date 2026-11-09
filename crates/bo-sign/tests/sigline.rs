//! Signature lines (CORE-SIGLINE-001): OOXML signatures that sign a signature
//! line carry the [MS-OFFCRYPTO] Office object (`SignatureInfoV1` with the line's
//! `SetupID`, `SignatureType` 2) and the `idValidSigLnImg` / `idInvalidSigLnImg`
//! objects; several lines of one document are signed one after the other and each
//! signature keeps verifying; an edit afterwards invalidates them all.
// Test helpers may unwrap: a panic is the desired failure mode here.
#![allow(clippy::unwrap_used)]

use bo_opc::{Limits, Package, Relationship, TargetMode, REL_OFFICE_DOCUMENT};
use bo_sign::ooxml::{verify_ooxml, verify_ooxml_each, SignatureLineSigning};
use bo_sign::pdf::SignOptions;
use bo_sign::report::verify_document;
use bo_sign::testing::self_signed;
use bo_sign::trust::TrustStore;
use bo_sign::{sign_document, SoftwareSigner};
use std::sync::OnceLock;
use std::time::{Duration, UNIX_EPOCH};

const ID1: &str = "{6F1B2C3D-4E5F-4061-8273-94A5B6C7D8E9}";
/// Signing time (Unix seconds).
const T0: u64 = 1_793_000_000;
const ID2: &str = "{0A1B2C3D-4E5F-4061-8273-94A5B6C7D8EA}";
const CT_DOCUMENT: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";

fn signer(n: usize) -> &'static SoftwareSigner {
    static S: OnceLock<[SoftwareSigner; 2]> = OnceLock::new();
    &S.get_or_init(|| {
        let mut rng = rand::thread_rng();
        [
            self_signed(&mut rng, "Nguyễn Văn A"),
            self_signed(&mut rng, "Trần Thị B"),
        ]
    })[n]
}

/// A document with two inline signature lines (ECMA-376 Part 4, §19.2.2.30).
fn two_lines() -> Vec<u8> {
    let line = |id: &str, rid: &str, who: &str| {
        format!(
            r#"<w:p><w:r><w:pict><v:shape id="L{rid}" style="width:192pt;height:96pt" stroked="f" filled="f"><v:imagedata r:id="{rid}" o:title=""/><o:signatureline v:ext="edit" id="{id}" provid="{{00000000-0000-0000-0000-000000000000}}" o:suggestedsigner="{who}" issignatureline="t"/></v:shape></w:pict></w:r></w:p>"#
        )
    };
    let doc = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office"><w:body>{}{}</w:body></w:document>"#,
        line(ID1, "rId5", "Nguyễn Văn A"),
        line(ID2, "rId6", "Trần Thị B")
    );
    let mut p = Package::new();
    p.add_part("/word/document.xml", CT_DOCUMENT, doc.into_bytes())
        .unwrap();
    for n in [1, 2] {
        p.add_part(
            &format!("/word/media/image{n}.png"),
            "image/png",
            vec![0x89, b'P', b'N', b'G', n],
        )
        .unwrap();
    }
    let img = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/image";
    let rel = |id: &str, ty: &str, target: &str| Relationship {
        id: id.into(),
        rel_type: ty.into(),
        target: target.into(),
        mode: TargetMode::Internal,
    };
    p.set_relationships(
        "/word/document.xml",
        &[
            rel("rId5", img, "media/image1.png"),
            rel("rId6", img, "media/image2.png"),
        ],
    );
    p.set_relationships(
        "/",
        &[rel("rId1", REL_OFFICE_DOCUMENT, "word/document.xml")],
    );
    p.write().unwrap()
}

fn sign_line(bytes: &[u8], n: usize, id: &str, text: &str) -> Vec<u8> {
    let opts = SignOptions {
        time: UNIX_EPOCH + Duration::from_secs(T0),
        signature_line: Some(SignatureLineSigning {
            setup_id: id.into(),
            text: text.into(),
            image: Vec::new(),
            comments: "Đồng ý & phê duyệt".into(),
            valid_image: vec![1, 2, 3],
            invalid_image: vec![4, 5],
        }),
        ..SignOptions::default()
    };
    sign_document(bytes, signer(n), &opts, None).unwrap()
}

fn part(bytes: &[u8], name: &str) -> String {
    let pkg = Package::read(bytes, &Limits::default()).unwrap();
    String::from_utf8(pkg.part(name).unwrap().data.clone()).unwrap()
}

#[test]
fn signing_a_line_references_its_id() {
    let signed = sign_line(&two_lines(), 0, ID1, "Nguyễn Văn A");
    let sigs = verify_ooxml(&signed).unwrap();
    assert_eq!(sigs.len(), 1);
    let line = sigs[0].signature_line.as_ref().unwrap();
    assert_eq!(line.setup_id, ID1);
    assert_eq!(line.text, "Nguyễn Văn A");
    assert_eq!(line.comments, "Đồng ý & phê duyệt");
    let xml = part(&signed, "/_xmlsignatures/sig1.xml");
    // [MS-OFFCRYPTO]: SignatureType 2 requires the two line pictures; base64 of the
    // bytes 1, 2, 3 is "AQID" and of 4, 5 is "BAU=" (RFC 4648 §4).
    assert!(xml.contains("<SignatureType>2</SignatureType>"));
    assert!(xml.contains("<Object Id=\"idValidSigLnImg\">AQID</Object>"));
    assert!(xml.contains("<Object Id=\"idInvalidSigLnImg\">BAU=</Object>"));
    for id in ["idOfficeObject", "idValidSigLnImg", "idInvalidSigLnImg"] {
        assert!(xml.contains(&format!("URI=\"#{id}\"")), "{id} is signed");
    }
    // The comment is escaped once in the canonical text.
    assert!(xml.contains("<SignatureComments>Đồng ý &amp; phê duyệt</SignatureComments>"));
    let report = verify_document(
        &signed,
        &TrustStore::new(),
        UNIX_EPOCH + Duration::from_secs(T0 + 100),
    )
    .unwrap();
    assert_eq!(report.signatures[0].signature_line.as_deref(), Some(ID1));
    assert_eq!(report.signature_lines.len(), 1);
    assert!(report.signature_lines[0].valid);
    assert_eq!(report.signature_lines[0].id, ID1);
}

#[test]
fn two_lines_signed_in_turn_both_verify_and_an_edit_invalidates_them() {
    let once = sign_line(&two_lines(), 0, ID1, "Nguyễn Văn A");
    let twice = sign_line(&once, 1, ID2, "Trần Thị B");
    let sigs = verify_ooxml(&twice).unwrap();
    assert_eq!(sigs.len(), 2);
    let ids: Vec<&str> = sigs
        .iter()
        .map(|s| s.signature_line.as_ref().unwrap().setup_id.as_str())
        .collect();
    assert_eq!(ids, [ID1, ID2]);
    assert!(sigs.iter().all(|s| s.unsigned_parts.is_empty()));
    let now = UNIX_EPOCH + Duration::from_secs(T0 + 100);
    let report = verify_document(&twice, &TrustStore::new(), now).unwrap();
    assert!(report.error.is_none());
    let lines: Vec<(&str, bool)> = report
        .signature_lines
        .iter()
        .map(|l| (l.id.as_str(), l.valid))
        .collect();
    assert_eq!(lines, [(ID1, true), (ID2, true)]);
    assert_eq!(
        report.signature_lines[1].text.as_deref(),
        Some("Trần Thị B")
    );

    // Edit the document after signing: every signature breaks.
    let mut pkg = Package::read(&twice, &Limits::default()).unwrap();
    let doc = part(&twice, "/word/document.xml").replace("Trần Thị B", "Trần Thị C");
    pkg.put_part("/word/document.xml", doc.into_bytes());
    let edited = pkg.write().unwrap();
    assert!(verify_ooxml(&edited).is_err());
    let each = verify_ooxml_each(&edited).unwrap();
    assert_eq!(each.len(), 2);
    assert!(each.iter().all(|p| p.result.is_err()));
    assert_eq!(each[1].setup_id.as_deref(), Some(ID2));
    let report = verify_document(&edited, &TrustStore::new(), now).unwrap();
    assert!(report.error.is_some());
    let lines: Vec<(&str, bool)> = report
        .signature_lines
        .iter()
        .map(|l| (l.id.as_str(), l.valid))
        .collect();
    assert_eq!(lines, [(ID1, false), (ID2, false)]);
    assert!(report.signature_lines[0]
        .error
        .as_deref()
        .unwrap()
        .contains("/word/document.xml"));
}

#[test]
fn a_signature_without_a_line_reports_none() {
    let opts = SignOptions {
        time: UNIX_EPOCH,
        ..SignOptions::default()
    };
    let signed = sign_document(&two_lines(), signer(0), &opts, None).unwrap();
    let sigs = verify_ooxml(&signed).unwrap();
    assert!(sigs[0].signature_line.is_none());
    let report = verify_document(&signed, &TrustStore::new(), UNIX_EPOCH).unwrap();
    assert_eq!(report.signature_lines.len(), 0);
    // A line signature needs the line id.
    let bad = SignOptions {
        signature_line: Some(SignatureLineSigning::default()),
        ..opts
    };
    assert!(sign_document(&two_lines(), signer(0), &bad, None).is_err());
}
