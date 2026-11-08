//! RFC 3161 time-stamps (PAdES B-T, XAdES-T), certificate path validation,
//! two-phase signing and the remote signer — all against a locally
//! generated PKI and in-process / loopback services (no network).
#![allow(clippy::unwrap_used)] // test helpers outside #[test] fns
use bo_sign::http::{HttpResponse, HttpTimestampClient};
use bo_sign::ooxml::{sign_ooxml_with, verify_ooxml};
use bo_sign::pdf::{sign_pdf_with, verify_pdf, SignOptions};
use bo_sign::remote::{JsonSigningApi, RemoteConfig, RemoteSigner};
use bo_sign::report::verify_document;
use bo_sign::testing::{validity_around, LocalTsa, TestPki, TsaFault};
use bo_sign::timestamp::{TimestampClient, TimestampRequest};
use bo_sign::trust::{validate_chain, ChainProblem, Purpose, TrustStore};
use bo_sign::{PreparedSignature, Signer as _, SoftwareSigner};
use pdf_writer::{Pdf, Rect, Ref};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DOCX: &[u8] = include_bytes!("../../../tests/corpus/doc/phase1-sample.docx");
const DAY: Duration = Duration::from_hours(24);

fn pki() -> &'static TestPki {
    static P: OnceLock<TestPki> = OnceLock::new();
    P.get_or_init(|| TestPki::new(&mut rand::thread_rng()))
}

/// Certificates valid from 10 days ago until yesterday.
fn expired_pki() -> &'static TestPki {
    static P: OnceLock<TestPki> = OnceLock::new();
    P.get_or_init(|| {
        let now = SystemTime::now();
        TestPki::with_validity(
            &mut rand::thread_rng(),
            validity_around(now - 5 * DAY, 5 * DAY, 4 * DAY),
        )
    })
}

fn clone_signer(s: &SoftwareSigner) -> SoftwareSigner {
    let (key, cert) = bo_sign::testing::to_pem(s);
    let mut pem = cert;
    for c in s.chain_der() {
        pem.push_str(&bo_sign::cert_to_pem(&c));
    }
    SoftwareSigner::from_pem(&key, pem.as_bytes()).unwrap()
}

fn tsa_at(p: &TestPki, t: SystemTime) -> LocalTsa {
    LocalTsa::new(clone_signer(&p.tsa), t)
}

fn store(p: &TestPki) -> TrustStore {
    let mut s = TrustStore::new();
    s.add_der(p.root.certificate_der()).unwrap();
    s
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

fn t0() -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(1_793_000_000)
}

#[test]
fn pdf_b_t_signature_carries_a_verified_timestamp() {
    let p = pki();
    let tsa = tsa_at(p, t0());
    let signed = sign_pdf_with(
        &sample_pdf(),
        &p.signer,
        &SignOptions::default(),
        Some(&tsa),
    )
    .unwrap();
    let sigs = verify_pdf(&signed).unwrap();
    assert_eq!(sigs.len(), 1);
    let ts = sigs[0].signer.timestamp.as_ref().expect("timestamp");
    assert_eq!(ts.gen_time, "2026-10-26T07:33:20Z");
    assert!(ts.tsa_subject.contains("Test TSA"));
    // The intermediate travels in the CMS certificate set.
    assert_eq!(sigs[0].signer.certificates.len(), 2);
}

#[test]
fn ooxml_xades_t_signature_carries_a_verified_timestamp() {
    let p = pki();
    let tsa = tsa_at(p, t0());
    let signed = sign_ooxml_with(DOCX, &p.signer, t0(), Some(&tsa)).unwrap();
    let sigs = verify_ooxml(&signed).unwrap();
    assert_eq!(sigs.len(), 1);
    assert_eq!(sigs[0].certificates.len(), 2);
    let ts = sigs[0].timestamp.as_ref().expect("timestamp");
    assert_eq!(ts.gen_time_unix, 1_793_000_000);

    let pkg = bo_opc::Package::read(&signed, &bo_opc::Limits::default()).unwrap();
    let xml =
        String::from_utf8(pkg.part("/_xmlsignatures/sig1.xml").unwrap().data.clone()).unwrap();
    assert!(xml.contains("<xd:SigningCertificate>"));
    assert!(xml.contains("<xd:EncapsulatedTimeStamp>"));

    // Tampering with the signed XAdES properties breaks the signature.
    let forged = xml.replace("<xd:SigningTime>2026", "<xd:SigningTime>2025");
    let mut pkg = pkg;
    pkg.put_part("/_xmlsignatures/sig1.xml", forged.into_bytes());
    let err = verify_ooxml(&pkg.write().unwrap()).unwrap_err().to_string();
    assert!(err.contains("idSignedProperties"), "{err}");
}

#[test]
fn misbehaving_tsa_is_rejected_when_signing() {
    let p = pki();
    for fault in [
        TsaFault::WrongNonce,
        TsaFault::WrongImprint,
        TsaFault::Reject,
    ] {
        let tsa = tsa_at(p, t0()).with_fault(fault);
        assert!(
            sign_pdf_with(
                &sample_pdf(),
                &p.signer,
                &SignOptions::default(),
                Some(&tsa)
            )
            .is_err(),
            "{fault:?}"
        );
    }
    // A token signed by a certificate without id-kp-timeStamping.
    let not_a_tsa = LocalTsa::new(clone_signer(&p.signer), t0());
    let err = sign_pdf_with(
        &sample_pdf(),
        &p.signer,
        &SignOptions::default(),
        Some(&not_a_tsa),
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("time-stamping"), "{err}");
}

#[test]
fn timestamp_over_another_signature_fails_verification() {
    let p = pki();
    let prepared = PreparedSignature::prepare(
        &sample_pdf(),
        p.signer.certificate_der(),
        &p.signer.chain_der(),
        &SignOptions::default(),
    )
    .unwrap();
    let sig = p.signer.sign_sha256(&prepared.digest().unwrap()).unwrap();
    let tsa = tsa_at(p, t0());
    let wrong = bo_sign::timestamp::timestamp(&tsa, b"something else", 7).unwrap();
    let signed = prepared.finish(&sig, Some(&wrong)).unwrap();
    let err = verify_pdf(&signed).unwrap_err().to_string();
    assert!(err.contains("imprint"), "{err}");
}

#[test]
fn two_phase_signing_with_browser_style_timestamp() {
    let p = pki();
    for doc in [sample_pdf(), DOCX.to_vec()] {
        let opts = SignOptions {
            time: t0(),
            ..SignOptions::default()
        };
        let prepared = PreparedSignature::prepare(
            &doc,
            p.signer.certificate_der(),
            &p.signer.chain_der(),
            &opts,
        )
        .unwrap();
        let sig = p.signer.sign_sha256(&prepared.digest().unwrap()).unwrap();
        // The caller builds the request itself (browser: fetch to the TSA).
        let req = TimestampRequest::new(&prepared.timestamp_message(&sig), 99).unwrap();
        let resp = tsa_at(p, SystemTime::now())
            .exchange(&req.to_der().unwrap())
            .unwrap();
        let token = req.accept_response(&resp).unwrap();
        let signed = prepared.finish(&sig, Some(&token)).unwrap();
        let report = verify_document(&signed, &store(p), SystemTime::now()).unwrap();
        assert_eq!(report.error, None);
        assert_eq!(report.signatures.len(), 1);
        assert!(report.signatures[0].chain.trusted, "{report:?}");
        assert!(
            report.signatures[0]
                .timestamp
                .as_ref()
                .unwrap()
                .chain
                .trusted
        );
    }
    // A time-stamp outside the TSA certificate validity is not trusted; the
    // signer path is then checked at the verification time.
    let prepared = PreparedSignature::prepare(
        &sample_pdf(),
        p.signer.certificate_der(),
        &p.signer.chain_der(),
        &SignOptions::default(),
    )
    .unwrap();
    let sig = p.signer.sign_sha256(&prepared.digest().unwrap()).unwrap();
    let token = bo_sign::timestamp::timestamp(&tsa_at(p, t0()), &sig, 5).unwrap();
    let signed = prepared.finish(&sig, Some(&token)).unwrap();
    let report = verify_document(&signed, &store(p), SystemTime::now()).unwrap();
    let s = &report.signatures[0];
    assert!(!s.timestamp.as_ref().unwrap().chain.trusted);
    assert!(s.chain.trusted);
    assert_ne!(s.validated_at, s.timestamp.as_ref().unwrap().time);
}

#[test]
fn chain_validation_reports_problems() {
    let p = pki();
    let leaf = p.signer.certificate_der();
    let pool = p.signer.chain_der();
    let now = SystemTime::now();
    let ok = validate_chain(leaf, &pool, &store(p), now, Purpose::DocumentSigning);
    assert!(ok.trusted, "{ok:?}");
    assert_eq!(ok.path.len(), 2);
    assert!(ok.anchor.as_deref().unwrap().contains("Test Root CA"));

    let none = validate_chain(
        leaf,
        &pool,
        &TrustStore::new(),
        now,
        Purpose::DocumentSigning,
    );
    assert!(!none.trusted);
    assert!(none.problems.contains(&ChainProblem::NoTrustAnchors));

    let missing = validate_chain(leaf, &[], &store(p), now, Purpose::DocumentSigning);
    assert!(
        matches!(missing.problems[0], ChainProblem::UnknownIssuer { .. }),
        "{missing:?}"
    );

    let other = validate_chain(
        leaf,
        &pool,
        &store(expired_pki()),
        now,
        Purpose::DocumentSigning,
    );
    assert!(!other.trusted);

    let later = validate_chain(
        leaf,
        &pool,
        &store(p),
        now + 3 * DAY,
        Purpose::DocumentSigning,
    );
    assert!(
        later
            .problems
            .iter()
            .any(|x| matches!(x, ChainProblem::Expired { .. })),
        "{later:?}"
    );
    let earlier = validate_chain(
        leaf,
        &pool,
        &store(p),
        now - 3 * DAY,
        Purpose::DocumentSigning,
    );
    assert!(earlier
        .problems
        .iter()
        .any(|x| matches!(x, ChainProblem::NotYetValid { .. })));

    let not_tsa = validate_chain(leaf, &pool, &store(p), now, Purpose::TimeStamping);
    assert!(not_tsa
        .problems
        .iter()
        .any(|x| matches!(x, ChainProblem::ExtendedKeyUsage { .. })));
    let tsa = validate_chain(
        p.tsa.certificate_der(),
        &[],
        &store(p),
        now,
        Purpose::TimeStamping,
    );
    assert!(tsa.trusted, "{tsa:?}");

    // An end-entity certificate cannot issue (basicConstraints cA = FALSE).
    let mut s = store(p);
    s.add_der(p.tsa.certificate_der()).unwrap();
    assert_eq!(s.len(), 2);
    assert!(
        validate_chain(b"garbage", &[], &s, now, Purpose::DocumentSigning)
            .problems
            .iter()
            .any(|x| matches!(x, ChainProblem::Malformed(_)))
    );
}

#[test]
fn trust_store_loads_pem_bundles() {
    let p = pki();
    let mut bundle = bo_sign::cert_to_pem(p.root.certificate_der());
    bundle.push_str(&bo_sign::cert_to_pem(p.intermediate.certificate_der()));
    let mut s = TrustStore::new();
    assert_eq!(s.add_bytes(bundle.as_bytes()).unwrap(), 2);
    assert_eq!(s.add_bytes(p.root.certificate_der()).unwrap(), 1);
    assert!(s.add_bytes(b"not a certificate").is_err());

    let dir = std::env::temp_dir().join(format!("bo-sign-trust-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("root.pem"), bundle).unwrap();
    std::fs::write(dir.join("ignored.txt"), "x").unwrap();
    let mut s = TrustStore::new();
    assert_eq!(s.add_path(&dir).unwrap(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn trusted_timestamp_keeps_signature_valid_after_expiry() {
    let p = expired_pki();
    let signed_at = SystemTime::now() - 5 * DAY;
    let tsa = tsa_at(p, signed_at);
    let signed = sign_pdf_with(
        &sample_pdf(),
        &p.signer,
        &SignOptions::default(),
        Some(&tsa),
    )
    .unwrap();
    let report = verify_document(&signed, &store(p), SystemTime::now()).unwrap();
    let s = &report.signatures[0];
    assert!(s.chain.trusted, "{s:?}");
    assert_eq!(s.validated_at, s.timestamp.as_ref().unwrap().time);

    // Without a time-stamp the expired certificate is reported.
    let plain = bo_sign::pdf::sign_pdf(&sample_pdf(), &p.signer, &SignOptions::default()).unwrap();
    let report = verify_document(&plain, &store(p), SystemTime::now()).unwrap();
    let s = &report.signatures[0];
    assert!(!s.chain.trusted);
    assert!(
        s.chain.problems.iter().any(|x| x.code == "expired"),
        "{s:?}"
    );
}

#[test]
fn report_flags_broken_signatures() {
    let p = pki();
    let mut signed =
        bo_sign::pdf::sign_pdf(&sample_pdf(), &p.signer, &SignOptions::default()).unwrap();
    let pos = signed.windows(3).position(|w| w == b"595").unwrap();
    signed[pos] = b'6';
    let report = verify_document(&signed, &store(p), SystemTime::now()).unwrap();
    assert!(report.error.is_some());
    assert!(verify_document(b"hello", &store(p), SystemTime::now()).is_err());
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["kind"], "pdf");
}

/// A JSON remote-signing service implemented as an in-process transport.
fn in_process_service(
    signer: SoftwareSigner,
) -> impl Fn(&str, &[(String, String)], &[u8]) -> bo_sign::Result<HttpResponse> {
    use base64::Engine as _;
    let b64 = base64::engine::general_purpose::STANDARD;
    move |url, _headers, body| {
        let req: serde_json::Value = serde_json::from_slice(body).unwrap();
        let json = if url.ends_with("/certificates") {
            serde_json::json!({ "certificates": [b64.encode(signer.certificate_der())] })
        } else {
            let digest: [u8; 32] = b64
                .decode(req["digest"].as_str().unwrap())
                .unwrap()
                .try_into()
                .unwrap();
            serde_json::json!({ "status": "signed", "signature": b64.encode(signer.sign_sha256(&digest).unwrap()) })
        };
        Ok(HttpResponse {
            status: 200,
            content_type: Some("application/json".into()),
            body: json.to_string().into_bytes(),
        })
    }
}

#[test]
fn remote_signer_with_in_process_transport() {
    let p = pki();
    let api = JsonSigningApi::new(
        RemoteConfig::new("https://sign.example.test/api"),
        Box::new(in_process_service(clone_signer(&p.signer))),
    );
    let remote = RemoteSigner::connect(Box::new(api)).unwrap();
    assert_eq!(remote.certificate_der(), p.signer.certificate_der());
    let signed = bo_sign::sign_document(DOCX, &remote, &SignOptions::default(), None).unwrap();
    assert_eq!(verify_ooxml(&signed).unwrap().len(), 1);

    // A service that signs with another key is caught.
    let other = in_process_service(clone_signer(&p.tsa));
    let lying = move |url: &str, h: &[(String, String)], b: &[u8]| {
        if url.ends_with("/certificates") {
            use base64::Engine as _;
            let json = serde_json::json!({
                "certificates": [base64::engine::general_purpose::STANDARD.encode(pki().signer.certificate_der())]
            });
            return Ok(HttpResponse {
                status: 200,
                content_type: None,
                body: json.to_string().into_bytes(),
            });
        }
        other(url, h, b)
    };
    let api = JsonSigningApi::new(RemoteConfig::new("https://x"), Box::new(lying));
    let remote = RemoteSigner::connect(Box::new(api)).unwrap();
    assert!(bo_sign::sign_document(DOCX, &remote, &SignOptions::default(), None).is_err());
}

#[cfg(feature = "http")]
mod over_http {
    use super::*;
    use bo_sign::http::UreqTransport;
    use bo_sign::testing::{mock_remote_service, mock_tsa_service};

    #[test]
    fn http_tsa_and_remote_signer_against_loopback_services() {
        let p = pki();
        let tsa_server = mock_tsa_service(tsa_at(p, t0())).unwrap();
        let tsa = HttpTimestampClient::new(
            format!("{}/tsr", tsa_server.url),
            Box::new(UreqTransport::default()),
        );
        let service = mock_remote_service(clone_signer(&p.signer), "secret", 2).unwrap();
        let mut config = RemoteConfig::new(format!("{}/api/v1/", service.url));
        config.access_token = Some("secret".into());
        config.credential_id = Some("cred-1".into());
        let remote = RemoteSigner::connect(Box::new(JsonSigningApi::new(
            config.clone(),
            Box::new(UreqTransport::default()),
        )))
        .unwrap();
        assert_eq!(remote.chain_der().len(), 1);
        let signed =
            bo_sign::sign_document(&sample_pdf(), &remote, &SignOptions::default(), Some(&tsa))
                .unwrap();
        let report = verify_document(&signed, &store(p), SystemTime::now()).unwrap();
        assert!(report.signatures[0].chain.trusted, "{report:?}");
        assert!(report.signatures[0].timestamp.is_some());

        // Wrong access token: HTTP 401 with the service message.
        config.access_token = Some("wrong".into());
        let err = RemoteSigner::connect(Box::new(JsonSigningApi::new(
            config,
            Box::new(UreqTransport::default()),
        )))
        .err()
        .unwrap()
        .to_string();
        assert!(err.contains("401") && err.contains("bad token"), "{err}");

        // A TSA that is not one.
        let bad = HttpTimestampClient::new(service.url.clone(), Box::new(UreqTransport::default()));
        assert!(bad.exchange(b"\x30\x00").is_err());
    }
}

#[cfg(feature = "pkcs12")]
#[test]
fn loads_pkcs12_files() {
    use p12_keystore::{Certificate, KeyStore, KeyStoreEntry, PrivateKey, PrivateKeyChain};
    let p = pki();
    let key = PrivateKey::from_der(&bo_sign::testing::key_pkcs8_der(&p.signer)).unwrap();
    let chain = PrivateKeyChain::new(
        b"k1".to_vec(),
        key,
        [
            Certificate::from_der(p.signer.certificate_der()).unwrap(),
            Certificate::from_der(p.intermediate.certificate_der()).unwrap(),
        ],
    );
    let mut ks = KeyStore::new();
    ks.add_entry("signer", KeyStoreEntry::PrivateKeyChain(chain));
    let p12 = ks.writer("mật khẩu").write().unwrap();
    let s = SoftwareSigner::from_pkcs12(&p12, "mật khẩu").unwrap();
    assert_eq!(s.certificate_der(), p.signer.certificate_der());
    assert_eq!(s.chain_der().len(), 1);
    assert!(SoftwareSigner::from_pkcs12(&p12, "sai").is_err());
    assert!(SoftwareSigner::from_pkcs12(b"garbage", "x").is_err());
}
