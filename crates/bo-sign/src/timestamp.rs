//! RFC 3161 Time-Stamp Protocol: requests (§2.4.1), responses (§2.4.2) and
//! token verification (`TSTInfo` in a CMS `SignedData`, §2.4.2).
//!
//! Used for PAdES baseline B-T (ETSI EN 319 142-1 §6.3, signature-time-stamp
//! attribute) and the XAdES `SignatureTimeStamp` of OOXML signatures
//! (ETSI TS 101 903 §7.3, [MS-OFFCRYPTO] §2.5.2).
//!
//! The transport is the [`TimestampClient`] trait: HTTP (RFC 3161 §3.4,
//! `application/timestamp-query`) in [`crate::http`], an in-process TSA in
//! [`crate::testing`].

use crate::cms_sig::verify_signed_data;
use crate::{iso_utc, sha256, HashAlg, Result, SignError};
use const_oid::db::{rfc5280, rfc5912};
use const_oid::ObjectIdentifier;
use der::asn1::{BitString, OctetString, Uint};
use der::{Any, Decode, Encode, Sequence, Tag, Tagged};
use spki::AlgorithmIdentifierOwned;
use x509_cert::ext::pkix::ExtendedKeyUsage;
use x509_cert::ext::Extension;

/// id-ct-TSTInfo (RFC 3161 §2.4.2).
pub const ID_CT_TST_INFO: ObjectIdentifier =
    ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.16.1.4");

/// MessageImprint ::= SEQUENCE { hashAlgorithm, hashedMessage } (§2.4.1).
#[derive(Clone, Debug, Eq, PartialEq, Sequence)]
pub struct MessageImprint {
    /// Hash algorithm of the imprint.
    pub hash_algorithm: AlgorithmIdentifierOwned,
    /// Hash of the time-stamped data.
    pub hashed_message: OctetString,
}

/// TimeStampReq (RFC 3161 §2.4.1).
#[derive(Clone, Debug, Eq, PartialEq, Sequence)]
pub struct TimeStampReq {
    /// Syntax version, 1.
    pub version: u8,
    /// Imprint of the data to time-stamp.
    pub message_imprint: MessageImprint,
    #[asn1(optional = "true")]
    /// Requested TSA policy.
    pub req_policy: Option<ObjectIdentifier>,
    #[asn1(optional = "true")]
    /// Replay protection; echoed in the token.
    pub nonce: Option<Uint>,
    #[asn1(default = "Default::default")]
    /// Ask the TSA to embed its certificate in the token.
    pub cert_req: bool,
    #[asn1(context_specific = "0", tag_mode = "IMPLICIT", optional = "true")]
    /// Request extensions.
    pub extensions: Option<Vec<Extension>>,
}

/// PKIStatusInfo (RFC 3161 §2.4.2).
#[derive(Clone, Debug, Eq, PartialEq, Sequence)]
pub struct PkiStatusInfo {
    /// PKIStatus: 0 granted, 1 grantedWithMods, 2 rejection, 3 waiting, 4/5 revocation.
    pub status: u8,
    #[asn1(optional = "true")]
    /// Human-readable status text.
    pub status_string: Option<Vec<String>>,
    #[asn1(optional = "true")]
    /// PKIFailureInfo bits.
    pub fail_info: Option<BitString>,
}

/// TimeStampResp (RFC 3161 §2.4.2); the token is kept as raw DER.
#[derive(Clone, Debug, Eq, PartialEq, Sequence)]
pub struct TimeStampResp {
    /// Whether the request was granted.
    pub status: PkiStatusInfo,
    #[asn1(optional = "true")]
    /// The token (`ContentInfo(SignedData(TSTInfo))`) when granted.
    pub time_stamp_token: Option<Any>,
}

/// Accuracy (RFC 3161 §2.4.2).
#[derive(Clone, Debug, Eq, PartialEq, Sequence)]
pub struct Accuracy {
    #[asn1(optional = "true")]
    /// Whole seconds.
    pub seconds: Option<u64>,
    #[asn1(context_specific = "0", tag_mode = "IMPLICIT", optional = "true")]
    /// Milliseconds (1..999).
    pub millis: Option<u16>,
    #[asn1(context_specific = "1", tag_mode = "IMPLICIT", optional = "true")]
    /// Microseconds (1..999).
    pub micros: Option<u16>,
}

/// TSTInfo (RFC 3161 §2.4.2). `genTime` is kept as raw `GeneralizedTime`
/// because TSAs may add fractional seconds, which strict DER time types reject.
#[derive(Clone, Debug, Eq, PartialEq, Sequence)]
pub struct TstInfo {
    /// Syntax version, 1.
    pub version: u8,
    /// TSA policy under which the token was issued.
    pub policy: ObjectIdentifier,
    /// Imprint copied from the request.
    pub message_imprint: MessageImprint,
    /// Serial number, unique per TSA.
    pub serial_number: Uint,
    /// Time of the time-stamp (`GeneralizedTime`, UTC).
    pub gen_time: Any,
    #[asn1(optional = "true")]
    /// Accuracy of `genTime`.
    pub accuracy: Option<Accuracy>,
    #[asn1(default = "Default::default")]
    /// Whether tokens of this TSA can be ordered by `genTime` alone.
    pub ordering: bool,
    #[asn1(optional = "true")]
    /// Nonce copied from the request.
    pub nonce: Option<Uint>,
    #[asn1(context_specific = "0", tag_mode = "EXPLICIT", optional = "true")]
    /// Name of the TSA (`GeneralName`).
    pub tsa: Option<Any>,
    #[asn1(context_specific = "1", tag_mode = "IMPLICIT", optional = "true")]
    /// Token extensions.
    pub extensions: Option<Vec<Extension>>,
}

/// Sends a DER `TimeStampReq` to a TSA and returns the DER `TimeStampResp`.
pub trait TimestampClient {
    /// Sends a DER `TimeStampReq` and returns the DER `TimeStampResp`.
    fn exchange(&self, request_der: &[u8]) -> Result<Vec<u8>>;
}

/// A request for a SHA-256 imprint of `message` (certReq = TRUE so the
/// token embeds the TSA certificate, needed for offline verification).
#[derive(Debug, Clone)]
pub struct TimestampRequest {
    req: TimeStampReq,
}

fn sha256_alg() -> AlgorithmIdentifierOwned {
    AlgorithmIdentifierOwned {
        oid: rfc5912::ID_SHA_256,
        parameters: None,
    }
}

fn positive_int(n: u64) -> Result<Uint> {
    // `Uint` strips leading zeros and adds the 0x00 sign byte DER needs.
    Ok(Uint::new(&n.to_be_bytes())?)
}

impl TimestampRequest {
    /// Request for the SHA-256 imprint of `message` with the given `nonce`.
    pub fn new(message: &[u8], nonce: u64) -> Result<Self> {
        Ok(Self {
            req: TimeStampReq {
                version: 1,
                message_imprint: MessageImprint {
                    hash_algorithm: sha256_alg(),
                    hashed_message: OctetString::new(sha256(message).to_vec())?,
                },
                req_policy: None,
                nonce: Some(positive_int(nonce)?),
                cert_req: true,
                extensions: None,
            },
        })
    }

    /// DER encoding of the `TimeStampReq`.
    pub fn to_der(&self) -> Result<Vec<u8>> {
        Ok(self.req.to_der()?)
    }

    /// Checks a DER `TimeStampResp` against this request (status granted,
    /// same imprint and nonce, valid token signature by a time-stamping
    /// certificate) and returns the DER token (`ContentInfo`).
    pub fn accept_response(&self, resp_der: &[u8]) -> Result<Vec<u8>> {
        let resp = TimeStampResp::from_der(resp_der)
            .map_err(|e| SignError::Malformed(format!("TimeStampResp: {e}")))?;
        // PKIStatus: 0 granted, 1 grantedWithMods.
        if resp.status.status > 1 {
            let text = resp
                .status
                .status_string
                .map(|s| s.join("; "))
                .unwrap_or_default();
            return Err(SignError::Signer(format!(
                "time-stamp authority refused the request (status {}{}{text})",
                resp.status.status,
                if text.is_empty() { "" } else { ": " }
            )));
        }
        let token = resp
            .time_stamp_token
            .ok_or_else(|| SignError::Malformed("TimeStampResp without token".into()))?
            .to_der()?;
        let (tst, _) = verify_token_parts(&token)?;
        if tst.message_imprint != self.req.message_imprint {
            return Err(SignError::Invalid(
                "time-stamp token imprint does not match the request".into(),
            ));
        }
        if tst.nonce != self.req.nonce {
            return Err(SignError::Invalid(
                "time-stamp token nonce does not match the request".into(),
            ));
        }
        Ok(token)
    }
}

/// Requests a token over `message` (SHA-256 imprint) and validates it.
pub fn timestamp(client: &dyn TimestampClient, message: &[u8], nonce: u64) -> Result<Vec<u8>> {
    let req = TimestampRequest::new(message, nonce)?;
    let resp = client.exchange(&req.to_der()?)?;
    req.accept_response(&resp)
}

/// A nonce for native callers (RFC 3161 §2.4.1: a large random number used
/// once). Derived from the clock, a process counter and the message; it only
/// has to be unpredictable enough to prevent replaying an old response.
/// Not for WASM (no clock there): pass a nonce from `crypto.getRandomValues`.
#[must_use]
pub fn fresh_nonce(message: &[u8]) -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let mut seed = nanos.to_be_bytes().to_vec();
    seed.extend_from_slice(&COUNTER.fetch_add(1, Ordering::Relaxed).to_be_bytes());
    seed.extend_from_slice(&std::process::id().to_be_bytes());
    seed.extend_from_slice(&sha256(message));
    let h = sha256(&seed);
    // Keep 63 bits so the INTEGER is small and positive.
    u64::from_be_bytes([h[0] & 0x7f, h[1], h[2], h[3], h[4], h[5], h[6], h[7]])
}

/// A verified RFC 3161 time-stamp token.
#[derive(Debug, Clone)]
pub struct TimestampInfo {
    /// `genTime` as Unix seconds (fraction dropped).
    pub gen_time_unix: u64,
    /// `genTime` as `YYYY-MM-DDThh:mm:ssZ`.
    pub gen_time: String,
    /// TSA certificate subject (RFC 4514).
    pub tsa_subject: String,
    /// DER of the TSA certificate.
    pub tsa_certificate_der: Vec<u8>,
    /// Every certificate embedded in the token (DER), for path building.
    pub certificates: Vec<Vec<u8>>,
    /// TSA policy OID (dotted).
    pub policy: String,
    /// Serial number (hex).
    pub serial: String,
}

/// Parses `YYYYMMDDHHMMSS[.f*]Z` (X.680 GeneralizedTime as constrained by
/// RFC 3161 §2.4.2: UTC, optional fraction) to Unix seconds.
pub(crate) fn parse_generalized_time(s: &str) -> Option<u64> {
    let s = s.strip_suffix('Z')?;
    let (main, frac) = s.split_once('.').unwrap_or((s, ""));
    if main.len() != 14
        || !main.bytes().all(|b| b.is_ascii_digit())
        || !frac.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let num = |a: usize, b: usize| main[a..b].parse::<i64>().ok();
    let (y, mo, d) = (num(0, 4)?, num(4, 6)?, num(6, 8)?);
    let (h, mi, sec) = (num(8, 10)?, num(10, 12)?, num(12, 14)?);
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || sec > 60 {
        return None;
    }
    // Days from civil (H. Hinnant, public domain).
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + h * 3600 + mi * 60 + sec).ok()
}

/// `GeneralizedTime` text of Unix seconds (`YYYYMMDDHHMMSSZ`).
#[must_use]
pub fn generalized_time(secs: u64) -> String {
    let (y, mo, d, h, mi, s) = crate::pdf::civil_from_unix(secs);
    format!("{y:04}{mo:02}{d:02}{h:02}{mi:02}{s:02}Z")
}

/// Verifies the CMS signature of a token and returns its `TSTInfo`.
fn verify_token_parts(token_der: &[u8]) -> Result<(TstInfo, TimestampInfo)> {
    let v = verify_signed_data(token_der, None)?;
    if v.econtent_type != ID_CT_TST_INFO {
        return Err(SignError::Malformed(
            "time-stamp token does not contain TSTInfo".into(),
        ));
    }
    let content = v
        .econtent
        .ok_or_else(|| SignError::Malformed("time-stamp token without TSTInfo".into()))?;
    let tst =
        TstInfo::from_der(&content).map_err(|e| SignError::Malformed(format!("TSTInfo: {e}")))?;
    // RFC 3161 §2.3: the TSA certificate carries id-kp-timeStamping.
    let eku = v
        .signer
        .tbs_certificate
        .get::<ExtendedKeyUsage>()
        .ok()
        .flatten();
    if !eku.is_some_and(|(_, e)| e.0.contains(&rfc5280::ID_KP_TIME_STAMPING)) {
        return Err(SignError::Invalid(
            "time-stamp token not signed by a time-stamping certificate".into(),
        ));
    }
    if tst.gen_time.tag() != Tag::GeneralizedTime {
        return Err(SignError::Malformed("TSTInfo genTime".into()));
    }
    let gen_text = std::str::from_utf8(tst.gen_time.value())
        .map_err(|_| SignError::Malformed("TSTInfo genTime".into()))?;
    let gen_time_unix = parse_generalized_time(gen_text)
        .ok_or_else(|| SignError::Malformed(format!("TSTInfo genTime {gen_text}")))?;
    let serial = tst
        .serial_number
        .as_bytes()
        .iter()
        .fold(String::new(), |mut s, b| {
            use std::fmt::Write as _;
            let _ = write!(s, "{b:02X}");
            s
        });
    let info = TimestampInfo {
        gen_time_unix,
        gen_time: iso_utc(gen_time_unix),
        tsa_subject: v.signer.tbs_certificate.subject.to_string(),
        tsa_certificate_der: v.signer.to_der()?,
        certificates: v.certificates,
        policy: tst.policy.to_string(),
        serial,
    };
    Ok((tst, info))
}

/// Verifies a DER token (`ContentInfo(SignedData(TSTInfo))`) over `message`:
/// token signature, `id-kp-timeStamping` and the message imprint.
pub fn verify_token(token_der: &[u8], message: &[u8]) -> Result<TimestampInfo> {
    let (tst, info) = verify_token_parts(token_der)?;
    let alg = HashAlg::from_digest_oid(tst.message_imprint.hash_algorithm.oid)?;
    if tst.message_imprint.hashed_message.as_bytes() != alg.digest(message).as_slice() {
        return Err(SignError::Invalid(
            "time-stamp imprint does not match the signature".into(),
        ));
    }
    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generalized_time_round_trip() {
        assert_eq!(parse_generalized_time("19700101000000Z"), Some(0));
        assert_eq!(
            parse_generalized_time("20261026073320Z"),
            Some(1_793_000_000)
        );
        assert_eq!(
            parse_generalized_time("20261026073320.125Z"),
            Some(1_793_000_000)
        );
        assert_eq!(generalized_time(1_793_000_000), "20261026073320Z");
        for bad in [
            "",
            "2026102607332Z",
            "20261326073320Z",
            "20261026073320",
            "2026x026073320Z",
        ] {
            assert_eq!(parse_generalized_time(bad), None, "{bad}");
        }
    }

    #[test]
    fn request_der_shape() {
        let req = TimestampRequest::new(b"abc", 0x1234).unwrap();
        let der = req.to_der().unwrap();
        let back = TimeStampReq::from_der(&der).unwrap();
        assert_eq!(back.version, 1);
        assert!(back.cert_req);
        assert_eq!(back.message_imprint.hash_algorithm.oid, rfc5912::ID_SHA_256);
        assert_eq!(
            back.message_imprint.hashed_message.as_bytes(),
            sha256(b"abc")
        );
        assert_ne!(fresh_nonce(b"x"), fresh_nonce(b"x"));
    }

    #[test]
    fn garbage_responses_are_errors() {
        let req = TimestampRequest::new(b"abc", 1).unwrap();
        assert!(req.accept_response(b"").is_err());
        assert!(req
            .accept_response(&[0x30, 0x03, 0x02, 0x01, 0x00])
            .is_err());
        // status = rejection (2)
        let rejected = TimeStampResp {
            status: PkiStatusInfo {
                status: 2,
                status_string: Some(vec!["bad alg".into()]),
                fail_info: None,
            },
            time_stamp_token: None,
        }
        .to_der()
        .unwrap();
        let err = req.accept_response(&rejected).unwrap_err().to_string();
        assert!(err.contains("bad alg"), "{err}");
        assert!(verify_token(b"\x30\x00", b"m").is_err());
    }
}
