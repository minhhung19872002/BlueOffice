//! OCSP (RFC 6960) in certificate path validation: responses built in-process
//! from a test PKI, and responses made by an independent implementation
//! (`openssl ocsp`, fixtures in `tests/data/ocsp`).
#![allow(clippy::unwrap_used)] // test helpers outside #[test] fns

use bo_sign::http::HttpResponse;
use bo_sign::ocsp::{self, CertStatus, OcspRequest};
use bo_sign::testing::{issue_crl, issue_ocsp, ocsp_responder, validity_around, TestPki};
use bo_sign::trust::{validate_chain, ChainProblem, OcspFetcher, Purpose, TrustStore};
use bo_sign::Signer as _;
use der::Decode;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use x509_cert::Certificate;

const DAY: Duration = Duration::from_hours(24);
const HOUR: Duration = Duration::from_hours(1);

fn pki() -> &'static TestPki {
    static P: OnceLock<TestPki> = OnceLock::new();
    P.get_or_init(|| TestPki::new(&mut rand::thread_rng()))
}

fn other_pki() -> &'static TestPki {
    static P: OnceLock<TestPki> = OnceLock::new();
    P.get_or_init(|| TestPki::new(&mut rand::thread_rng()))
}

fn store(p: &TestPki, responses: &[Vec<u8>]) -> TrustStore {
    let mut s = TrustStore::new();
    s.add_der(p.root.certificate_der()).unwrap();
    for r in responses {
        s.add_ocsp_der(r).unwrap();
    }
    s
}

fn check(s: &TrustStore, at: SystemTime) -> bo_sign::trust::ChainReport {
    let p = pki();
    validate_chain(
        p.signer.certificate_der(),
        &p.signer.chain_der(),
        s,
        at,
        Purpose::DocumentSigning,
    )
}

/// OCSP response about the test signer (serial 3, issued by the intermediate).
fn about_signer(
    responder: &bo_sign::SoftwareSigner,
    status: CertStatus,
    this: SystemTime,
    next: Option<SystemTime>,
) -> Vec<u8> {
    let p = pki();
    issue_ocsp(
        responder,
        p.signer.certificate_der(),
        p.intermediate.certificate_der(),
        status,
        this,
        next,
    )
}

// source: RFC 6960 §4.2.1 (BasicOCSPResponse), §4.2.2.2 (authorized responders)
#[test]
fn issuer_signed_responses() {
    let p = pki();
    let now = SystemTime::now();
    let next = Some(now + DAY);
    let good = about_signer(&p.intermediate, CertStatus::Good, now - HOUR, next);
    assert!(check(&store(p, std::slice::from_ref(&good)), now).trusted);

    let revoked = about_signer(
        &p.intermediate,
        CertStatus::Revoked(now - HOUR),
        now - HOUR,
        next,
    );
    let r = check(&store(p, std::slice::from_ref(&revoked)), now);
    assert!(!r.trusted);
    assert!(
        matches!(&r.problems[..], [ChainProblem::Revoked { subject, .. }] if subject.contains("Nguyễn Văn A")),
        "{r:?}"
    );
    // Validated (time-stamped) before the revocation: still trusted.
    assert!(check(&store(p, &[revoked]), now - 2 * HOUR).trusted);

    // A good OCSP status does not hide a CRL revocation, but makes an
    // out-of-date CRL harmless.
    let crl = issue_crl(&p.intermediate, &[(3, now - HOUR)], now - HOUR, next);
    let mut s = store(p, std::slice::from_ref(&good));
    s.add_crl_der(&crl).unwrap();
    assert!(matches!(
        &check(&s, now).problems[..],
        [ChainProblem::Revoked { .. }]
    ));
    let stale = issue_crl(&p.intermediate, &[], now - 3 * HOUR, Some(now - HOUR));
    let mut s = store(p, &[good]);
    s.add_crl_der(&stale).unwrap();
    assert!(check(&s, now).trusted);
}

#[test]
fn delegated_and_forged_responders() {
    let p = pki();
    let now = SystemTime::now();
    let next = Some(now + DAY);
    let validity = validity_around(now, DAY, DAY);
    let revoked = |responder: &bo_sign::SoftwareSigner| {
        about_signer(responder, CertStatus::Revoked(now - HOUR), now - HOUR, next)
    };
    // Delegated by the issuer with id-kp-OCSPSigning: accepted.
    let delegated = ocsp_responder(&mut rand::thread_rng(), &p.intermediate, true, validity);
    let r = check(&store(p, &[revoked(&delegated)]), now);
    assert!(
        matches!(&r.problems[..], [ChainProblem::Revoked { .. }]),
        "{r:?}"
    );
    // Without the EKU, by another CA's key with the same name, or by the root
    // (not the issuer): ignored.
    let no_eku = ocsp_responder(&mut rand::thread_rng(), &p.intermediate, false, validity);
    let foreign = ocsp_responder(
        &mut rand::thread_rng(),
        &other_pki().intermediate,
        true,
        validity,
    );
    for responder in [&no_eku, &foreign, &other_pki().intermediate, &p.root] {
        assert!(check(&store(p, &[revoked(responder)]), now).trusted);
    }
    // A response about another certificate is ignored.
    let other = issue_ocsp(
        &p.intermediate,
        p.tsa.certificate_der(),
        p.intermediate.certificate_der(),
        CertStatus::Revoked(now - HOUR),
        now - HOUR,
        next,
    );
    assert!(check(&store(p, &[other]), now).trusted);
}

#[test]
fn fetcher_and_http_transport() {
    let p = pki();
    let now = SystemTime::now();
    let calls = Arc::new(AtomicUsize::new(0));
    let resp = about_signer(
        &p.intermediate,
        CertStatus::Revoked(now - HOUR),
        now - HOUR,
        Some(now + DAY),
    );
    // The fetcher is asked for each certificate without a loaded response.
    let mut s = store(p, &[]);
    let (c2, r2) = (calls.clone(), resp.clone());
    s.set_ocsp_fetcher(OcspFetcher(Arc::new(move |_, _| {
        c2.fetch_add(1, Ordering::SeqCst);
        Some(r2.clone())
    })));
    let r = check(&s, now);
    assert!(
        matches!(&r.problems[..], [ChainProblem::Revoked { .. }]),
        "{r:?}"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2, "signer and intermediate");

    // HTTP POST application/ocsp-request (RFC 6960 Appendix A.1).
    let cert = Certificate::from_der(p.signer.certificate_der()).unwrap();
    let issuer = Certificate::from_der(p.intermediate.certificate_der()).unwrap();
    let want = ocsp::cert_id(&cert, &issuer).unwrap();
    let transport = move |url: &str, headers: &[(String, String)], body: &[u8]| {
        assert_eq!(url, "http://ocsp.test/");
        assert!(headers
            .iter()
            .any(|(k, v)| k == "Content-Type" && v == "application/ocsp-request"));
        let req = OcspRequest::from_der(body).unwrap();
        assert_eq!(req.tbs_request.request_list[0].req_cert, want);
        Ok(HttpResponse {
            status: 200,
            content_type: Some("application/ocsp-response".into()),
            body: resp.clone(),
        })
    };
    let der = ocsp::fetch(&transport, "http://ocsp.test/", &cert, &issuer).unwrap();
    let basic = ocsp::parse_response(&der).unwrap();
    let st = ocsp::status(&basic, &cert, &issuer).unwrap();
    assert!(matches!(st.status, CertStatus::Revoked(_)));
    let fail = |_: &str, _: &[(String, String)], _: &[u8]| {
        Ok(HttpResponse {
            status: 500,
            content_type: None,
            body: Vec::new(),
        })
    };
    assert!(ocsp::fetch(&fail, "http://ocsp.test/", &cert, &issuer).is_err());
    // Not a successful response.
    assert!(ocsp::parse_response(&[0x30, 0x03, 0x0a, 0x01, 0x01]).is_err());
}

fn fixture(name: &str) -> Vec<u8> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/ocsp");
    std::fs::read(dir.join(name)).unwrap()
}

fn pem_cert(name: &str) -> Vec<u8> {
    let text = String::from_utf8(fixture(name)).unwrap();
    bo_sign::pem_blocks(&text, "CERTIFICATE").unwrap().remove(0)
}

/// 2026-11-08T12:00:00Z: inside the fixtures' validity, after Bình's revocation.
fn fixture_time() -> SystemTime {
    UNIX_EPOCH + Duration::from_hours(498_372) // 1_794_139_200 s
}

// Interoperability: responses made by `openssl ocsp` (OpenSSL 3.0) for a root →
// issuing CA → An (good) / Bình (revoked) PKI; one signed by the issuing CA,
// one by a delegated responder certificate.
#[test]
fn openssl_responses() {
    let mut base = TrustStore::new();
    base.add_der(&pem_cert("root.pem")).unwrap();
    let pool = vec![pem_cert("inter.pem")];
    let at = fixture_time();
    let run = |leaf: &str, responses: &[&str]| {
        let mut s = base.clone();
        for r in responses {
            s.add_ocsp_der(&fixture(r)).unwrap();
        }
        validate_chain(&pem_cert(leaf), &pool, &s, at, Purpose::DocumentSigning)
    };
    assert!(run("an.pem", &["resp-an.der"]).trusted);
    assert!(run("an.pem", &["resp-an-delegated.der"]).trusted);
    let r = run("binh.pem", &["resp-binh.der"]);
    assert!(
        matches!(&r.problems[..], [ChainProblem::Revoked { subject, date }] if subject.contains("Binh") && date.starts_with("2026-11-08")),
        "{r:?}"
    );
    // The responses themselves verify and give the expected status.
    let inter = Certificate::from_der(&pem_cert("inter.pem")).unwrap();
    let status = |leaf: &str, resp: &str| {
        let cert = Certificate::from_der(&pem_cert(leaf)).unwrap();
        let basic = ocsp::parse_response(&fixture(resp)).unwrap();
        ocsp::status(&basic, &cert, &inter).map(|c| c.status)
    };
    assert_eq!(status("an.pem", "resp-an.der"), Some(CertStatus::Good));
    assert_eq!(
        status("an.pem", "resp-an-delegated.der"),
        Some(CertStatus::Good)
    );
    assert!(matches!(
        status("binh.pem", "resp-binh.der"),
        Some(CertStatus::Revoked(_))
    ));
    assert_eq!(status("an.pem", "resp-binh.der"), None);
    // Bình's response does not apply to An.
    assert!(run("an.pem", &["resp-binh.der"]).trusted);
    // Responder URL from the Authority Information Access extension.
    let chi = Certificate::from_der(&pem_cert("chi.pem")).unwrap();
    assert_eq!(ocsp::responder_urls(&chi), ["http://ocsp.example.vn/ca"]);
    let an = Certificate::from_der(&pem_cert("an.pem")).unwrap();
    assert!(ocsp::responder_urls(&an).is_empty());
    // DER responses load through add_bytes / add_path too.
    let mut s = TrustStore::new();
    assert_eq!(s.add_bytes(&fixture("resp-an.der")).unwrap(), 1);
    assert_eq!(s.ocsp_count(), 1);
}
