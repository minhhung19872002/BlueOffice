//! End-to-end signing tests with a throwaway self-signed certificate.
use bo_sign::ooxml::{sign_ooxml, verify_ooxml};
use bo_sign::pdf::{sign_pdf, verify_pdf, SignOptions};
use bo_sign::testing::self_signed;
use bo_sign::{Signer as _, SoftwareSigner};
use pdf_writer::{Pdf, Rect, Ref};
use std::sync::OnceLock;
use std::time::{Duration, UNIX_EPOCH};

fn signer() -> &'static SoftwareSigner {
    static S: OnceLock<SoftwareSigner> = OnceLock::new();
    S.get_or_init(|| self_signed(&mut rand::thread_rng(), "Nguyễn Văn A"))
}

/// Two-page PDF with a nested page tree, written like `bo-pdf` does.
fn sample_pdf() -> Vec<u8> {
    let mut pdf = Pdf::new();
    let (cat, root, inner, p1, p2) = (
        Ref::new(1),
        Ref::new(2),
        Ref::new(3),
        Ref::new(4),
        Ref::new(5),
    );
    pdf.catalog(cat).pages(root);
    pdf.pages(root).kids([inner]).count(2);
    pdf.pages(inner).parent(root).kids([p1, p2]).count(2);
    for p in [p1, p2] {
        pdf.page(p)
            .parent(inner)
            .media_box(Rect::new(0.0, 0.0, 595.0, 842.0));
    }
    pdf.finish()
}

fn opts() -> SignOptions {
    SignOptions {
        name: Some("Nguyễn Văn A".into()),
        reason: Some("Phê duyệt".into()),
        location: Some("Hà Nội".into()),
        time: UNIX_EPOCH + Duration::from_secs(1_793_000_000),
        ..SignOptions::default()
    }
}

#[test]
fn pdf_sign_and_verify() {
    let original = sample_pdf();
    let signed = sign_pdf(&original, signer(), &opts()).unwrap();
    assert!(
        signed.starts_with(&original),
        "incremental update keeps the original bytes"
    );
    let sigs = verify_pdf(&signed).unwrap();
    assert_eq!(sigs.len(), 1);
    assert!(sigs[0].covers_whole_file);
    assert!(sigs[0].signer.subject.contains("Bluestar Test"));

    // A conforming reader sees the field on page 1 and in the AcroForm.
    let doc = lopdf::Document::load_mem(&signed).unwrap();
    let catalog = doc.catalog().unwrap();
    let acro = catalog.get(b"AcroForm").unwrap();
    let acro = doc.dereference(acro).unwrap().1.as_dict().unwrap();
    assert_eq!(acro.get(b"SigFlags").unwrap().as_i64().unwrap(), 3);
    let page1 = doc.get_object((4, 0)).unwrap().as_dict().unwrap();
    assert_eq!(page1.get(b"Annots").unwrap().as_array().unwrap().len(), 1);
}

#[test]
fn pdf_second_signature_keeps_first_valid() {
    let once = sign_pdf(&sample_pdf(), signer(), &opts()).unwrap();
    let twice = sign_pdf(&once, signer(), &opts()).unwrap();
    let sigs = verify_pdf(&twice).unwrap();
    assert_eq!(sigs.len(), 2);
    assert!(!sigs[0].covers_whole_file);
    assert!(sigs[1].covers_whole_file);
    let doc = lopdf::Document::load_mem(&twice).unwrap();
    let page1 = doc.get_object((4, 0)).unwrap().as_dict().unwrap();
    assert_eq!(page1.get(b"Annots").unwrap().as_array().unwrap().len(), 2);
}

#[test]
fn pdf_tampering_is_detected() {
    let mut signed = sign_pdf(&sample_pdf(), signer(), &opts()).unwrap();
    let pos = signed.windows(3).position(|w| w == b"595").unwrap();
    signed[pos] = b'6';
    assert!(verify_pdf(&signed).is_err());
}

#[test]
fn pdf_reserve_too_small_is_an_error() {
    let o = SignOptions {
        reserve_hex: 64,
        ..opts()
    };
    assert!(sign_pdf(&sample_pdf(), signer(), &o).is_err());
}

const DOCX: &[u8] = include_bytes!("../../../tests/corpus/doc/phase1-sample.docx");

#[test]
fn ooxml_sign_and_verify() {
    let t = UNIX_EPOCH + Duration::from_secs(1_793_000_000);
    let signed = sign_ooxml(DOCX, signer(), t).unwrap();
    let sigs = verify_ooxml(&signed).unwrap();
    assert_eq!(sigs.len(), 1);
    assert_eq!(sigs[0].part, "/_xmlsignatures/sig1.xml");
    assert_eq!(
        sigs[0].signing_time.as_deref(),
        Some("2026-10-26T07:33:20Z")
    );
    assert!(verify_ooxml(DOCX).unwrap().is_empty());

    // Second signature: both verify (origin rels are not part of the manifest).
    let twice = sign_ooxml(&signed, signer(), t).unwrap();
    assert_eq!(verify_ooxml(&twice).unwrap().len(), 2);
}

#[test]
fn ooxml_tampering_is_detected() {
    let signed = sign_ooxml(DOCX, signer(), UNIX_EPOCH).unwrap();
    let mut pkg = bo_opc::Package::read(&signed, &bo_opc::Limits::default()).unwrap();
    let doc = pkg.part("/word/document.xml").unwrap().data.clone();
    let mut changed = doc.clone();
    changed.extend_from_slice(b"<!-- x -->");
    pkg.put_part("/word/document.xml", changed);
    let err = verify_ooxml(&pkg.write().unwrap()).unwrap_err();
    assert!(err.to_string().contains("/word/document.xml"), "{err}");
}

#[test]
fn loads_pem_key_and_certificate() {
    let (key, cert) = bo_sign::testing::to_pem(signer());
    let s = SoftwareSigner::from_pem(&key, cert.as_bytes()).unwrap();
    let signed = sign_ooxml(DOCX, &s, UNIX_EPOCH).unwrap();
    assert_eq!(verify_ooxml(&signed).unwrap().len(), 1);
    // DER certificate works too; a mismatched key is rejected.
    assert!(SoftwareSigner::from_pem(&key, signer().certificate_der()).is_ok());
    let other = self_signed(&mut rand::thread_rng(), "Khác");
    let (other_key, _) = bo_sign::testing::to_pem(&other);
    assert!(SoftwareSigner::from_pem(&other_key, cert.as_bytes()).is_err());
    assert!(SoftwareSigner::from_pem("garbage", cert.as_bytes()).is_err());
}

// source: ECMA-376 Part 2 §13 (package signatures, §13.2.4.24 relationship transform), W3C XML-DSig 1.1,
// W3C Exclusive C14N 1.0, ETSI TS 101 903 §7.2.2 — fixture signed by lxml + OpenSSL
// (tests/data/foreign/make_foreign_ooxml.py), not by BlueOffice
#[test]
fn ooxml_signatures_of_other_producers_verify() {
    use bo_sign::report::verify_document;
    use bo_sign::trust::TrustStore;
    const FOREIGN: &[u8] = include_bytes!("data/foreign/foreign-signed.docx");
    const CERT: &[u8] = include_bytes!("data/foreign/foreign-cert.der");
    let sigs = verify_ooxml(FOREIGN).unwrap();
    assert_eq!(sigs.len(), 1);
    assert_eq!(sigs[0].certificate_der, CERT);
    assert_eq!(
        sigs[0].signing_time.as_deref(),
        Some("2026-11-08T12:00:00Z")
    );
    // word/styles.xml is outside the signature.
    assert_eq!(sigs[0].unsigned_parts, vec!["/word/styles.xml".to_owned()]);
    let mut store = TrustStore::new();
    store.add_der(CERT).unwrap();
    let r = verify_document(FOREIGN, &store, std::time::SystemTime::now()).unwrap();
    assert!(r.error.is_none(), "{:?}", r.error);
    assert!(r.signatures[0].chain.trusted, "{:?}", r.signatures[0].chain);
    assert!(!r.signatures[0].covers_whole_document);
    // A signed part changed: detected.
    let mut pkg = bo_opc::Package::read(FOREIGN, &bo_opc::Limits::default()).unwrap();
    let doc = pkg.part("/word/document.xml").unwrap().data.clone();
    let changed = String::from_utf8(doc)
        .unwrap()
        .replacen("</w:body>", " </w:body>", 1);
    pkg.add_part(
        "/word/document.xml",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
        changed.into_bytes(),
    )
    .unwrap();
    let err = verify_ooxml(&pkg.write().unwrap()).unwrap_err().to_string();
    assert!(err.contains("document.xml"), "{err}");
}
