//! Signature lines (CORE-SIGLINE-001): a VML picture shape carrying
//! `o:signatureline` (ECMA-376 Part 4, Office VML `signatureline`; listed as
//! `CT_SignatureLine` in the Office drawing VML schema) that marks where a person
//! is asked to sign. The signature itself is an OOXML package signature
//! (ECMA-376 Part 2, §12) whose `SignatureInfoV1/SetupID` ([MS-OFFCRYPTO]
//! "idOfficeObject Object Element") names the line's `id`.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Attributes of `o:signatureline` plus the signed state found in the package
/// signatures when the document was read (display only: never written to the
/// document part and left out of equality and hash, so content fingerprints —
/// which decide what is written back verbatim — and the "modified" state do not
/// see it).
#[derive(Debug, Clone, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignatureLine {
    /// `id`: GUID of the line, with braces (`ST_Guid`).
    pub id: String,
    /// `provid`: GUID of the signature provider (`{00000000-…}` = default).
    #[serde(default)]
    pub provider_id: String,
    /// `issignatureline` (default true).
    #[serde(default = "yes")]
    pub is_signature_line: bool,
    /// `o:suggestedsigner`: first line under the rule (name).
    #[serde(default)]
    pub suggested_signer: String,
    /// `o:suggestedsigner2`: second line (title).
    #[serde(default)]
    pub suggested_signer2: String,
    /// `o:suggestedsigneremail`.
    #[serde(default)]
    pub suggested_signer_email: String,
    /// `signinginstructions`: text shown to the signer.
    #[serde(default)]
    pub signing_instructions: String,
    /// `allowcomments`: the signer may add a comment.
    #[serde(default)]
    pub allow_comments: bool,
    /// `showsigndate` (default true): the signed line shows the date.
    #[serde(default = "yes")]
    pub show_sign_date: bool,
    /// Signed state (from the package signatures; not part of the line).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed: Option<SignedLine>,
}

impl SignatureLine {
    /// Everything but the signed state.
    #[allow(clippy::type_complexity)]
    fn key(&self) -> (&str, &str, bool, &str, &str, &str, &str, bool, bool) {
        (
            &self.id,
            &self.provider_id,
            self.is_signature_line,
            &self.suggested_signer,
            &self.suggested_signer2,
            &self.suggested_signer_email,
            &self.signing_instructions,
            self.allow_comments,
            self.show_sign_date,
        )
    }
}

impl PartialEq for SignatureLine {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl std::hash::Hash for SignatureLine {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // `signed` is display state (see the type's documentation).
        self.key().hash(state);
    }
}

fn yes() -> bool {
    true
}

/// What a signature of a line says ([MS-OFFCRYPTO] `SignatureInfoV1`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignedLine {
    /// `SignatureText`: the typed name (empty when an image was used).
    #[serde(default)]
    pub text: String,
    /// `SignatureImage` bytes (PNG / JPEG), when the signer chose a picture.
    #[serde(skip)]
    pub image: Option<Arc<Vec<u8>>>,
    /// Signing date as `YYYY-MM-DD` (from `SignatureTime`), empty when unknown.
    #[serde(default)]
    pub date: String,
    /// Verification result pushed by the host (`None`: not verified yet).
    #[serde(default)]
    pub valid: Option<bool>,
}

impl SignatureLine {
    /// Media key under which hosts register the signature image of line `id`
    /// (an engine image, never a package part).
    #[must_use]
    pub fn image_key(id: &str) -> String {
        format!("sigline:{id}")
    }

    /// The line without its signed state (what the document part stores).
    #[must_use]
    pub fn unsigned(&self) -> SignatureLine {
        SignatureLine {
            signed: None,
            ..self.clone()
        }
    }
}
