//! Cryptographic constraints over time (ETSI EN 319 102-1 §5.2.8 "cryptographic verification" and
//! §5.6.2.5 "past signature validation": an algorithm that has become weak is only acceptable for a
//! signature, certificate or time-stamp with a proof of existence from before the algorithm's sunset).
//!
//! The sunset dates follow the public NIST SP 800-131A transition (SHA-1 for digital signature generation
//! and RSA keys shorter than 2048 bits disallowed from 2014-01-01); RSA keys shorter than 1024 bits are never
//! accepted. ETSI TS 119 312 gives comparable dates for qualified signatures. Signing never uses SHA-1.

use crate::{HashAlg, Result, SignError};
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Algorithm sunsets of a trust store: the defaults below, overridable per deployment (a national scheme may
/// set other dates). Names: `SHA-1`, `RSA-1024` (keys of 1024–2047 bits), `RSA-512` (shorter keys).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Policy {
    overrides: BTreeMap<&'static str, Option<SystemTime>>,
}

impl Policy {
    /// Overrides the sunset of `name` (`None`: never weak).
    pub fn set_sunset(&mut self, name: &str, sunset: Option<SystemTime>) -> Result<()> {
        let key = match name.to_ascii_uppercase().as_str() {
            "SHA-1" | "SHA1" => "SHA-1",
            "RSA-1024" => "RSA-1024",
            "RSA-512" => "RSA-512",
            _ => {
                return Err(SignError::Unsupported(format!(
                    "algorithm '{name}' (SHA-1, RSA-1024, RSA-512)"
                )))
            }
        };
        self.overrides.insert(key, sunset);
        Ok(())
    }

    fn get(&self, key: &'static str, default: Option<SystemTime>) -> Option<SystemTime> {
        self.overrides.get(key).copied().unwrap_or(default)
    }

    /// Sunset of a hash under this policy.
    #[must_use]
    pub fn hash_sunset(&self, alg: HashAlg) -> Option<SystemTime> {
        match alg {
            HashAlg::Sha1 => self.get("SHA-1", hash_sunset(alg)),
            _ => hash_sunset(alg),
        }
    }

    /// Sunset of an RSA key size under this policy.
    #[must_use]
    pub fn rsa_sunset(&self, bits: usize) -> Option<SystemTime> {
        if bits < 1024 {
            self.get("RSA-512", rsa_sunset(bits))
        } else if bits < 2048 {
            self.get("RSA-1024", rsa_sunset(bits))
        } else {
            None
        }
    }
}

/// 2014-01-01T00:00:00Z.
const SUNSET_2014: u64 = 1_388_534_400;

/// The time from which `alg` is no longer acceptable (`None`: acceptable today).
#[must_use]
pub fn hash_sunset(alg: HashAlg) -> Option<SystemTime> {
    match alg {
        HashAlg::Sha1 => Some(UNIX_EPOCH + Duration::from_secs(SUNSET_2014)),
        HashAlg::Sha256 | HashAlg::Sha384 | HashAlg::Sha512 => None,
    }
}

/// The time from which an RSA key of `bits` is no longer acceptable (`None`: acceptable today).
#[must_use]
pub fn rsa_sunset(bits: usize) -> Option<SystemTime> {
    if bits < 1024 {
        Some(UNIX_EPOCH)
    } else if bits < 2048 {
        Some(UNIX_EPOCH + Duration::from_secs(SUNSET_2014))
    } else {
        None
    }
}

/// Name of a hash for reports.
#[must_use]
pub fn hash_name(alg: HashAlg) -> &'static str {
    match alg {
        HashAlg::Sha1 => "SHA-1",
        HashAlg::Sha256 => "SHA-256",
        HashAlg::Sha384 => "SHA-384",
        HashAlg::Sha512 => "SHA-512",
    }
}

/// `Some(sunset)` when something with this sunset is weak at the validation time `at`
/// (no proof of existence before the sunset).
#[must_use]
pub fn weak_at(sunset: Option<SystemTime>, at: SystemTime) -> Option<SystemTime> {
    sunset.filter(|s| at >= *s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha1_and_short_keys_are_weak_only_after_their_sunset() {
        let before = UNIX_EPOCH + Duration::from_secs(SUNSET_2014 - 86_400);
        let after = UNIX_EPOCH + Duration::from_secs(SUNSET_2014 + 86_400);
        assert_eq!(weak_at(hash_sunset(HashAlg::Sha1), before), None);
        assert!(weak_at(hash_sunset(HashAlg::Sha1), after).is_some());
        assert_eq!(weak_at(hash_sunset(HashAlg::Sha256), after), None);
        assert_eq!(weak_at(rsa_sunset(1024), before), None);
        assert!(weak_at(rsa_sunset(1024), after).is_some());
        assert!(weak_at(rsa_sunset(512), before).is_some());
        assert_eq!(weak_at(rsa_sunset(2048), after), None);
    }

    #[test]
    fn a_deployment_can_move_or_lift_a_sunset() {
        let mut p = Policy::default();
        let later = UNIX_EPOCH + Duration::from_secs(SUNSET_2014 + 3 * 365 * 86_400);
        p.set_sunset("sha1", Some(later)).unwrap();
        p.set_sunset("RSA-1024", None).unwrap();
        let mid = UNIX_EPOCH + Duration::from_secs(SUNSET_2014 + 86_400);
        assert_eq!(weak_at(p.hash_sunset(HashAlg::Sha1), mid), None);
        assert!(weak_at(p.hash_sunset(HashAlg::Sha1), later).is_some());
        assert_eq!(weak_at(p.rsa_sunset(1024), SystemTime::now()), None);
        assert!(weak_at(p.rsa_sunset(512), mid).is_some());
        assert!(p.set_sunset("MD5", None).is_err());
    }
}
