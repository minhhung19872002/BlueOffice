//! ECDSA certificates (RFC 5480, RFC 5758 §3.2): a P-256 root issuing a P-384 end entity, validated like
//! RSA paths (`ecdsa-with-SHA256` / `ecdsa-with-SHA384` certificate signatures).
#![allow(clippy::unwrap_used)]

use bo_sign::trust::{validate_chain, Purpose, TrustStore};
use der::Encode;
use std::str::FromStr;
use std::time::{Duration, SystemTime};
use x509_cert::builder::{Builder, CertificateBuilder, Profile};
use x509_cert::name::Name;
use x509_cert::serial_number::SerialNumber;
use x509_cert::spki::{EncodePublicKey, SubjectPublicKeyInfoOwned};
use x509_cert::time::Validity;

fn name(cn: &str) -> Name {
    Name::from_str(&format!("CN={cn},O=Bluestar Test,C=VN")).unwrap()
}

fn spki(der: &[u8]) -> SubjectPublicKeyInfoOwned {
    SubjectPublicKeyInfoOwned::try_from(der).unwrap()
}

/// DER `(root, leaf)`.
fn ec_chain() -> (Vec<u8>, Vec<u8>) {
    let mut rng = rand::thread_rng();
    let root_key = p256::ecdsa::SigningKey::random(&mut rng);
    let leaf_key = p384::ecdsa::SigningKey::random(&mut rng);
    let validity = Validity::from_now(Duration::from_hours(24)).unwrap();
    let root_spki = spki(
        root_key
            .verifying_key()
            .to_public_key_der()
            .unwrap()
            .as_bytes(),
    );
    let root = CertificateBuilder::new(
        Profile::Root,
        SerialNumber::from(1u32),
        validity,
        name("EC Root"),
        root_spki,
        &root_key,
    )
    .unwrap()
    .build::<p256::ecdsa::DerSignature>()
    .unwrap();
    let leaf_spki = spki(
        leaf_key
            .verifying_key()
            .to_public_key_der()
            .unwrap()
            .as_bytes(),
    );
    let leaf = CertificateBuilder::new(
        Profile::Leaf {
            issuer: name("EC Root"),
            enable_key_agreement: false,
            enable_key_encipherment: false,
        },
        SerialNumber::from(2u32),
        validity,
        name("Trần Thị B"),
        leaf_spki,
        &root_key,
    )
    .unwrap()
    .build::<p256::ecdsa::DerSignature>()
    .unwrap();
    (root.to_der().unwrap(), leaf.to_der().unwrap())
}

// source: RFC 5480 §2 (EC subject public keys), RFC 5758 §3.2 (ecdsa-with-SHA256), FIPS 186-4 §6.4
#[test]
fn ecdsa_paths_validate_and_a_changed_certificate_does_not() {
    let (root, leaf) = ec_chain();
    let mut store = TrustStore::new();
    store.add_der(&root).unwrap();
    let r = validate_chain(
        &leaf,
        &[],
        &store,
        SystemTime::now(),
        Purpose::DocumentSigning,
    );
    assert!(r.trusted, "{:?}", r.problems);
    assert_eq!(r.anchor.as_deref(), Some("CN=EC Root,O=Bluestar Test,C=VN"));
    // One byte of the signature value changed (the last one, inside the DER INTEGER s): no longer verifies.
    let mut bad = leaf.clone();
    let last = bad.len() - 1;
    bad[last] ^= 0x01;
    let r = validate_chain(
        &bad,
        &[],
        &store,
        SystemTime::now(),
        Purpose::DocumentSigning,
    );
    assert!(!r.trusted);
}

/// P-256 root and a P-256 (or P-384) end-entity signer it issues.
fn ec_signer(p384_leaf: bool) -> (Vec<u8>, bo_sign::SoftwareSigner) {
    let mut rng = rand::thread_rng();
    let root_key = p256::ecdsa::SigningKey::random(&mut rng);
    let validity = Validity::from_now(Duration::from_hours(24)).unwrap();
    let root_spki = spki(
        root_key
            .verifying_key()
            .to_public_key_der()
            .unwrap()
            .as_bytes(),
    );
    let root = CertificateBuilder::new(
        Profile::Root,
        SerialNumber::from(1u32),
        validity,
        name("EC Root"),
        root_spki,
        &root_key,
    )
    .unwrap()
    .build::<p256::ecdsa::DerSignature>()
    .unwrap()
    .to_der()
    .unwrap();
    let profile = Profile::Leaf {
        issuer: name("EC Root"),
        enable_key_agreement: false,
        enable_key_encipherment: false,
    };
    let leaf = |leaf_spki| {
        CertificateBuilder::new(
            profile.clone(),
            SerialNumber::from(5u32),
            validity,
            name("Lê Văn C"),
            leaf_spki,
            &root_key,
        )
        .unwrap()
        .build::<p256::ecdsa::DerSignature>()
        .unwrap()
        .to_der()
        .unwrap()
    };
    let signer = if p384_leaf {
        let k = p384::ecdsa::SigningKey::random(&mut rng);
        let cert = leaf(spki(
            k.verifying_key().to_public_key_der().unwrap().as_bytes(),
        ));
        bo_sign::SoftwareSigner::new_p384(k, cert)
    } else {
        let k = p256::ecdsa::SigningKey::random(&mut rng);
        let cert = leaf(spki(
            k.verifying_key().to_public_key_der().unwrap().as_bytes(),
        ));
        bo_sign::SoftwareSigner::new_p256(k, cert)
    };
    (root, signer)
}

fn sample_pdf() -> Vec<u8> {
    use pdf_writer::{Pdf, Rect, Ref};
    let mut pdf = Pdf::new();
    pdf.catalog(Ref::new(1)).pages(Ref::new(2));
    pdf.pages(Ref::new(2)).kids([Ref::new(3)]).count(1);
    pdf.page(Ref::new(3))
        .parent(Ref::new(2))
        .media_box(Rect::new(0.0, 0.0, 595.0, 842.0));
    pdf.finish()
}

// source: RFC 5758 §3.2 (ecdsa-with-SHA256 in CMS), ETSI EN 319 142-1 (PAdES), XML-DSig 1.1 §6.4.3
// (ecdsa-sha256, r || s), RFC 5915 / RFC 5208 (EC private keys)
#[test]
fn ecdsa_keys_sign_pdf_and_ooxml_that_verify_as_trusted() {
    use bo_sign::report::verify_document;
    use bo_sign::Signer as _;
    const DOCX: &[u8] = include_bytes!("../../../tests/corpus/doc/phase1-sample.docx");
    for p384 in [false, true] {
        let (root, signer) = ec_signer(p384);
        let mut store = TrustStore::new();
        store.add_der(&root).unwrap();
        // PAdES.
        let pdf = bo_sign::pdf::sign_pdf(
            &sample_pdf(),
            &signer,
            &bo_sign::pdf::SignOptions::default(),
        )
        .unwrap();
        let r = verify_document(&pdf, &store, SystemTime::now()).unwrap();
        assert!(r.error.is_none(), "{:?}", r.error);
        assert!(r.signatures[0].chain.trusted, "{:?}", r.signatures[0].chain);
        // OOXML: ecdsa-sha256 SignatureMethod, 2 × field size SignatureValue.
        let docx = bo_sign::ooxml::sign_ooxml(DOCX, &signer, SystemTime::now()).unwrap();
        let sigs = bo_sign::ooxml::verify_ooxml(&docx).unwrap();
        assert_eq!(sigs.len(), 1);
        let r = verify_document(&docx, &store, SystemTime::now()).unwrap();
        assert!(r.error.is_none() && r.signatures[0].chain.trusted, "{r:?}");
        // The key round-trips through PKCS#8 PEM.
        let (key_pem, cert_pem) = bo_sign::testing::to_pem(&signer);
        let again = bo_sign::SoftwareSigner::from_pem(&key_pem, cert_pem.as_bytes()).unwrap();
        assert_eq!(again.certificate_der(), signer.certificate_der());
    }
}

#[test]
fn raw_ecdsa_signatures_from_tokens_become_der() {
    use bo_sign::Signer as _;
    let (_, signer) = ec_signer(false);
    let der_sig = signer.sign_sha256(&[7u8; 32]).unwrap();
    let raw = bo_sign::ecdsa_der_to_raw(&der_sig, bo_sign::KeyKind::EcP256).unwrap();
    assert_eq!(raw.len(), 64);
    assert_eq!(
        bo_sign::normalize_signature(signer.certificate_der(), &raw).unwrap(),
        der_sig
    );
    assert_eq!(
        bo_sign::normalize_signature(signer.certificate_der(), &der_sig).unwrap(),
        der_sig
    );
}
