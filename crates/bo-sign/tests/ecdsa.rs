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
