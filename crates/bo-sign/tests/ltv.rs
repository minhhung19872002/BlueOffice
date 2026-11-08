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

// source: ETSI EN 319 142-1 §6.5 (B-LTA), ISO 32000-2 §12.8.5 (document time-stamp)
#[test]
fn document_timestamps_cover_signatures_and_dss() {
    use bo_sign::pdf::{add_document_timestamp, verify_pdf_timestamps};
    let p = pki();
    let now = SystemTime::now();
    let tsa = LocalTsa::new(clone_signer(&p.tsa), now);
    let mut online = anchors();
    online
        .add_crl_der(&issue_crl(
            &p.intermediate,
            &[],
            now - HOUR,
            Some(now + DAY),
        ))
        .unwrap();
    online
        .add_crl_der(&issue_crl(&p.root, &[], now - HOUR, Some(now + DAY)))
        .unwrap();
    // B-LT, then B-LTA.
    let lt = add_validation_data(&signed(), &online, now).unwrap();
    let lta = add_document_timestamp(&lt, &tsa).unwrap();
    assert!(lta.starts_with(&lt), "incremental update");
    let sig = &verify_pdf(&lta).unwrap()[0];
    assert!(!sig.covers_whole_file && sig.only_validation_data_after);
    let ts = verify_pdf_timestamps(&lta).unwrap();
    assert_eq!(ts.len(), 1);
    assert!(ts[0].covers_whole_file);
    assert!(ts[0].info.tsa_subject.contains("Test TSA"));
    let r = verify_document(&lta, &anchors(), now).unwrap();
    assert!(r.error.is_none(), "{:?}", r.error);
    assert!(r.signatures[0].covers_whole_document && r.signatures[0].chain.trusted);
    assert_eq!(r.document_timestamps.len(), 1);
    assert!(r.document_timestamps[0].covers_whole_document);
    assert!(r.document_timestamps[0].timestamp.chain.trusted);

    // Renewal: validation data for the time-stamp's TSA, then a new time-stamp.
    let data = collect_validation_data(&lta, &online, now).unwrap();
    assert!(data.certs.contains(&p.tsa.certificate_der().to_vec()));
    let renewed = add_document_timestamp(&add_dss(&lta, &data).unwrap(), &tsa).unwrap();
    let ts = verify_pdf_timestamps(&renewed).unwrap();
    assert_eq!(ts.len(), 2);
    assert!(ts[0].only_validation_data_after && ts[1].covers_whole_file);
    assert!(verify_pdf(&renewed).unwrap()[0].only_validation_data_after);

    // Any other later change is caught for the signature and the time-stamps:
    // a final DSS update whose dictionary gains a key PAdES does not allow.
    let extra = Dss {
        certs: vec![b"extra".to_vec()],
        ..Dss::default()
    };
    let mut tampered = add_dss(&renewed, &extra).unwrap();
    let pos = tampered.windows(6).rposition(|w| w == b"/Certs").unwrap();
    tampered[pos + 5] = b'z';
    let ts = verify_pdf_timestamps(&tampered).unwrap();
    assert!(ts
        .iter()
        .all(|t| !t.covers_whole_file && !t.only_validation_data_after));
    assert!(!verify_pdf(&tampered).unwrap()[0].only_validation_data_after);
    // A covered byte changed: the newest time-stamp's imprint no longer matches.
    let pos = renewed
        .windows(13)
        .rposition(|w| w == b"/Type /Catalo")
        .unwrap();
    let mut broken = renewed.clone();
    broken[pos + 13] = b'X';
    assert!(verify_pdf_timestamps(&broken).is_err());
}
