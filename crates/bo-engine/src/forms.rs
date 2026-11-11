//! Legacy form fields and the "filling in forms" restriction over the command bridge
//! (ECMA-376 Part 1, §17.16.17 ffData, §17.15.1.29 documentProtection, §17.6.6 formProt).
//!
//! While filling in forms is enforced, a command is let through when it does not change the
//! document, when the selection is in an unprotected section, or — for typing and deleting —
//! when the selection is in a form field that can be filled in (or a content control); Tab moves
//! between the fields. Anything else is refused with [`PROTECTED`].

use crate::edit::Command;
use bo_edit::forms::FormsAccess;
use bo_edit::Session;
use bo_model::form::{DocProtection, PROTECT_FORMS};
use serde::Serialize;

/// Error of a command refused by the editing restriction.
pub const PROTECTED: &str = "forms:protected";
/// Error when leaving a number field holding something else.
pub const BAD_NUMBER: &str = "forms:number";
/// Error when leaving a date field holding something else.
pub const BAD_DATE: &str = "forms:date";
/// Error of a wrong password when stopping the protection.
pub const WRONG_PASSWORD: &str = "forms:password";

/// The editing restriction, for hosts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectionInfo {
    /// §17.18 `ST_DocProtect` (`forms`, `readOnly`, `comments`, `trackedChanges`, `none`).
    pub edit: String,
    /// The restriction is enforced.
    pub enforced: bool,
    /// A password is needed to stop it.
    pub has_password: bool,
    /// Sections left editable while filling in forms (§17.6.6 formProt).
    pub unprotected_sections: Vec<usize>,
    /// Number of sections.
    pub sections: usize,
}

/// The restriction state of the session (`None` when the document has none).
#[must_use]
pub fn info(s: &Session) -> Option<ProtectionInfo> {
    let doc = s.doc();
    let unprotected: Vec<usize> = doc
        .sections
        .iter()
        .enumerate()
        .filter(|(_, x)| x.form_unprotected)
        .map(|(i, _)| i)
        .collect();
    let p = doc.protection.as_ref();
    (p.is_some() || !unprotected.is_empty()).then(|| ProtectionInfo {
        edit: p.map(|p| p.edit.clone()).unwrap_or_default(),
        enforced: p.is_some_and(|p| p.enforced),
        has_password: p.is_some_and(DocProtection::has_password),
        unprotected_sections: unprotected,
        sections: doc.sections.len(),
    })
}

/// The command to run in place of `cmd` while the restriction holds, or the refusal.
pub(crate) fn gate(cmd: Command, s: &Session) -> Result<Command, &'static str> {
    use Command as C;
    if !s.forms_locked() {
        return Ok(cmd);
    }
    // Commands that do not change the document, and filling in.
    let passive = matches!(
        cmd,
        C::State
            | C::SetSelection { .. }
            | C::Click { .. }
            | C::DragTo { .. }
            | C::Move { .. }
            | C::SelectAll
            | C::Find { .. }
            | C::SelectMatch { .. }
            | C::Copy
            | C::Review
            | C::SetAuthor { .. }
            | C::SetRevisionView { .. }
            | C::ShowFieldCodes { .. }
            | C::NextRevision { .. }
            | C::SelectComment { .. }
            | C::NextComment { .. }
            | C::CrossReferenceTargets
            | C::MergeData { .. }
            | C::MergePreview { .. }
            | C::SetSavePassword { .. }
            | C::SaveFormat { .. }
            | C::Spelling { .. }
            | C::SpellAt { .. }
            | C::SpellIgnore { .. }
            | C::SpellAddWord { .. }
            | C::EquationTemplates
            | C::Equation
            | C::AutoCorrect { .. }
            | C::SetSpelling { .. }
            | C::SetGrammar { .. }
            | C::GrammarIgnore { .. }
            | C::Notes
            | C::FontList
            | C::StyleList
            | C::NewFromTemplate { .. }
            | C::Undo
            | C::Redo
            | C::PickFormat
            | C::CancelFormat
            | C::UpdateFields { .. }
            | C::NextField { .. }
            | C::FieldPrompts
            | C::SetControlValue { .. }
            | C::SetFormFieldValue { .. }
            | C::NextFormField { .. }
            | C::UnprotectDocument { .. }
            | C::CloseHeaderFooter
            | C::CloseTextBox
    );
    if passive {
        return Ok(cmd);
    }
    let access = s.forms_access();
    match (cmd, access) {
        (c, FormsAccess::Free)
        | (
            c @ (C::InsertText { .. } | C::DeleteBackward | C::DeleteForward | C::Paste { .. }),
            FormsAccess::Control,
        ) => Ok(c),
        (C::Tab { back }, _) => Ok(C::NextFormField { back }),
        (
            C::InsertText { text }
            | C::Paste {
                text: Some(text), ..
            },
            FormsAccess::Text(_),
        ) => Ok(C::FormType { text }),
        (C::DeleteBackward, FormsAccess::Text(_)) => Ok(C::FormDelete { back: true }),
        (C::DeleteForward, FormsAccess::Text(_)) => Ok(C::FormDelete { back: false }),
        // Space toggles a check box.
        (C::InsertText { text }, FormsAccess::Choice(_)) if text == " " => {
            Ok(C::SetFormFieldValue {
                value: String::new(),
            })
        }
        _ => Err(PROTECTED),
    }
}

/// Start the restriction `edit` (default "filling in forms") with an optional password (an
/// iterated SHA-512 verifier, [MS-OFFCRYPTO] §2.4.2.4) and the sections left editable.
pub(crate) fn protect(
    s: &mut Session,
    edit: &str,
    password: Option<&str>,
    unprotected: Option<&[usize]>,
) -> Result<(), String> {
    if let Some(u) = unprotected {
        s.set_unprotected_sections(u).map_err(|e| e.to_string())?;
    }
    let mut attrs = Vec::new();
    if let Some(pw) = password.filter(|p| !p.is_empty()) {
        let h = bo_crypto::protection::new_protection_hash(pw).map_err(|e| e.to_string())?;
        attrs = vec![
            ("algorithmName".to_owned(), h.algorithm_name),
            ("hashValue".to_owned(), h.hash_value),
            ("saltValue".to_owned(), h.salt_value),
            ("spinCount".to_owned(), h.spin_count.to_string()),
        ];
    }
    s.set_protection(Some(DocProtection {
        edit: if edit.is_empty() {
            PROTECT_FORMS.to_owned()
        } else {
            edit.to_owned()
        },
        enforced: true,
        formatting: false,
        attrs,
    }));
    // §17.18.37 trackedChanges: every change is tracked while the protection holds.
    if s.tracking_locked() {
        s.set_track_changes(true);
    }
    Ok(())
}

/// Stop the restriction when `password` matches its verifier (the restriction type is kept,
/// not enforced). A legacy verifier (`w:hash` / `w:salt`, §17.15.1.29 legacy attributes) cannot
/// be checked yet, so such a protection is refused rather than accepting any password.
pub(crate) fn unprotect(s: &mut Session, password: Option<&str>) -> Result<(), &'static str> {
    let Some(p) = s.protection().cloned() else {
        return Ok(());
    };
    if let (Some(alg), Some(hash), Some(salt)) = (
        p.attr("algorithmName"),
        p.attr("hashValue"),
        p.attr("saltValue"),
    ) {
        let h = bo_crypto::protection::ProtectionHash {
            algorithm_name: alg.to_owned(),
            hash_value: hash.to_owned(),
            salt_value: salt.to_owned(),
            spin_count: p
                .attr("spinCount")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
        };
        if !bo_crypto::protection::verify_protection(password.unwrap_or_default(), &h) {
            return Err(WRONG_PASSWORD);
        }
    } else if p.attr("hash").is_some() || p.attr("cryptProviderType").is_some() {
        return Err(crate::security::UNSUPPORTED_HASH);
    }
    s.set_protection(Some(DocProtection {
        edit: p.edit,
        enforced: false,
        formatting: p.formatting,
        attrs: Vec::new(),
    }));
    Ok(())
}

#[cfg(test)]
mod legacy_password_tests {
    use super::{unprotect, WRONG_PASSWORD};
    use bo_edit::Session;
    use bo_model::{DocProtection, Document};

    fn protected(attrs: &[(&str, &str)]) -> Session {
        let mut s = Session::new(Document::default());
        s.set_protection(Some(DocProtection {
            edit: "forms".into(),
            enforced: true,
            formatting: false,
            attrs: attrs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        }));
        s
    }

    /// A legacy verifier (`w:hash` / `w:salt`) cannot be checked: stopping is refused and the
    /// restriction stays enforced, whatever password is given.
    #[test]
    fn legacy_verifier_is_refused_not_bypassed() {
        let mut s = protected(&[
            ("cryptProviderType", "rsaFull"),
            ("hash", "AbCd"),
            ("salt", "EfGh"),
        ]);
        assert_eq!(
            unprotect(&mut s, Some("anything")),
            Err(crate::security::UNSUPPORTED_HASH)
        );
        assert_eq!(
            unprotect(&mut s, None),
            Err(crate::security::UNSUPPORTED_HASH)
        );
        assert!(s.protection().is_some_and(|p| p.enforced));
    }

    #[test]
    fn no_password_stops_and_a_wrong_modern_one_is_rejected() {
        let mut s = protected(&[]);
        assert_eq!(unprotect(&mut s, None), Ok(()));
        assert!(s.protection().is_some_and(|p| !p.enforced));

        let h = bo_crypto::protection::new_protection_hash("đúng").unwrap();
        let mut s = protected(&[
            ("algorithmName", h.algorithm_name.as_str()),
            ("hashValue", h.hash_value.as_str()),
            ("saltValue", h.salt_value.as_str()),
            ("spinCount", &h.spin_count.to_string()),
        ]);
        assert_eq!(unprotect(&mut s, Some("sai")), Err(WRONG_PASSWORD));
        assert_eq!(unprotect(&mut s, Some("đúng")), Ok(()));
    }
}
