//! PAdES baseline B-LT (ETSI EN 319 142-1 §6.4): long-term validation data in
//! the Document Security Store, added after signing without breaking the
//! signature, and used when verifying offline.
#![allow(clippy::unwrap_used)] // test helpers outside #[test] fns

use bo_sign::ocsp::CertStatus;
use bo_sign::pdf::{add_dss, read_dss, sign_pdf_with, verify_pdf, Dss, SignOptions};
use bo_sign::report::{add_validation_data, collect_validation_data, verify_document};
use bo_sign::testing::{issue_crl, issue_ocsp, LocalTsa, TestPki};
use bo_sign::trust::{OcspFetcher, TrustStore};
use bo_sign::{Signer as _, SoftwareSigner};
use pdf_writer::{Pdf, Rect, Ref};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime};

const HOUR: Duration = Duration::from_hours(1);
const DAY: Duration = Duration::from_hours(24);

fn pki() -> &'static TestPki {
    static P: OnceLock<TestPki> = OnceLock::new();
    P.get_or_init(|| TestPki::new(&mut rand::thread_rng()))
}

fn clone_signer(s: &SoftwareSigner) -> SoftwareSigner {
    let (key, cert) = bo_sign::testing::to_pem(s);
    let mut pem = cert;
    for c in s.chain_der() {
        pem.push_str(&bo_sign::cert_to_pem(&c));
    }
    SoftwareSigner::from_pem(&key, pem.as_bytes()).unwrap()
}

fn sample_pdf() -> Vec<u8> {
    let mut pdf = Pdf::new();
    pdf.catalog(Ref::new(1)).pages(Ref::new(2));
    pdf.pages(Ref::new(2)).kids([Ref::new(3)]).count(1);
    pdf.page(Ref::new(3))
        .parent(Ref::new(2))
        .media_box(Rect::new(0.0, 0.0, 595.0, 842.0));
    pdf.finish()
}

/// Signed (B-T, time-stamped an hour ago) sample PDF.
fn signed() -> Vec<u8> {
    let p = pki();
    let tsa = LocalTsa::new(clone_signer(&p.tsa), SystemTime::now() - HOUR);
    sign_pdf_with(
        &sample_pdf(),
        &p.signer,
        &SignOptions::default(),
        Some(&tsa),
    )
    .unwrap()
}

fn anchors() -> TrustStore {
    let mut s = TrustStore::new();
    s.add_der(pki().root.certificate_der()).unwrap();
    s
}

// source: ETSI EN 319 142-1 §5.4.2.2 (DSS), ISO 32000-2 §12.8.4.3
#[test]
fn dss_round_trip_and_signature_still_covers_the_document() {
    let pdf = signed();
    assert_eq!(read_dss(&pdf).unwrap(), Dss::default());
    let data = Dss {
        certs: vec![b"cert-1".to_vec(), b"cert-2".to_vec()],
        crls: vec![vec![0, 1, 2, 255, b'\n', b'\r']],
        ocsps: Vec::new(),
    };
    let ltv = add_dss(&pdf, &data).unwrap();
    assert!(ltv.starts_with(&pdf), "incremental update");
    assert_eq!(read_dss(&ltv).unwrap(), data);
    let sigs = verify_pdf(&ltv).unwrap();
    assert!(!sigs[0].covers_whole_file && sigs[0].only_validation_data_after);
    // Adding again: only new items, existing ones kept once.
    let more = Dss {
        certs: vec![b"cert-2".to_vec(), b"cert-3".to_vec()],
        ..Dss::default()
    };
    let ltv2 = add_dss(&ltv, &more).unwrap();
    let all = read_dss(&ltv2).unwrap();
    assert_eq!(all.certs.len(), 3);
    assert_eq!(all.crls, data.crls);
    assert!(verify_pdf(&ltv2).unwrap()[0].only_validation_data_after);
    assert_eq!(add_dss(&ltv2, &more).unwrap(), ltv2, "nothing new");

    // Any other change after the signature is reported as such.
    let pos = ltv
        .windows(13)
        .rposition(|w| w == b"/Type /Catalo")
        .unwrap();
    let mut tampered = ltv.clone();
    tampered[pos + 13] = b'X';
    let sig = &verify_pdf(&tampered).unwrap()[0];
    assert!(!sig.covers_whole_file && !sig.only_validation_data_after);
}

#[test]
fn validation_data_makes_revocation_checkable_offline() {
    let p = pki();
    let now = SystemTime::now();
    let pdf = signed();
    // Online: the issuing CA's CRL revokes the signer 30 minutes ago (after the
    // time-stamp), the root's CRL is empty; OCSP answers for the TSA.
    let mut online = anchors();
    let crl = issue_crl(
        &p.intermediate,
        &[(3, now - HOUR / 2)],
        now - HOUR / 2,
        Some(now + DAY),
    );
    online.add_crl_der(&crl).unwrap();
    online
        .add_crl_der(&issue_crl(&p.root, &[], now - HOUR, Some(now + DAY)))
        .unwrap();
    let tsa_ocsp = issue_ocsp(
        &p.root,
        p.tsa.certificate_der(),
        p.root.certificate_der(),
        CertStatus::Good,
        now - HOUR,
        Some(now + DAY),
    );
    let tsa_ocsp2 = tsa_ocsp.clone();
    online.set_ocsp_fetcher(OcspFetcher(Arc::new(move |c, _| {
        (c.tbs_certificate.subject.to_string().contains("TSA")).then(|| tsa_ocsp2.clone())
    })));
    let data = collect_validation_data(&pdf, &online, now).unwrap();
    // Signer, issuing CA, root, TSA.
    assert_eq!(data.certs.len(), 4, "{}", data.certs.len());
    assert!(data.crls.contains(&crl));
    assert_eq!(data.ocsps, vec![tsa_ocsp]);
    let ltv = add_validation_data(&pdf, &online, now).unwrap();

    // Offline, anchors only: the revocation after the time-stamp does not
    // invalidate the signature, the DSS answers every revocation question.
    let r = verify_document(&ltv, &anchors(), now).unwrap();
    let s = &r.signatures[0];
    assert!(s.covers_whole_document);
    assert!(s.chain.trusted, "{:?}", s.chain);
    assert!(s.timestamp.as_ref().unwrap().chain.trusted);

    // A revocation before the time-stamp, carried only by the DSS, is found.
    let early = issue_crl(
        &p.intermediate,
        &[(3, now - 2 * HOUR)],
        now - HOUR,
        Some(now + DAY),
    );
    let mut online = anchors();
    online.add_crl_der(&early).unwrap();
    let ltv = add_validation_data(&pdf, &online, now).unwrap();
    let r = verify_document(&ltv, &anchors(), now).unwrap();
    let s = &r.signatures[0];
    assert!(!s.chain.trusted);
    assert_eq!(s.chain.problems[0].code, "revoked");
    // Without the DSS the same check passes: the data really came from the file.
    assert!(
        verify_document(&pdf, &anchors(), now).unwrap().signatures[0]
            .chain
            .trusted
    );
}
