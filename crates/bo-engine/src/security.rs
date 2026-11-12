//! File-level protection shared by BlueDoc, BlueSheet and BlueSlide
//! (CORE-SEC-001).
//!
//! * Mark as final — the custom file property `_MarkAsFinal` (`vt:bool`
//!   true, ECMA-376 Part 1, §22.3 custom properties): the file opens
//!   read-only with a notice until the user chooses to edit it anyway, which
//!   removes the property on the next save.
//! * Read-only recommended and password to modify — `w:writeProtection`
//!   (§17.15.1.93, `w:recommended`) in WordprocessingML settings,
//!   `fileSharing` (§18.2.12, `readOnlyRecommended`, `reservationPassword` /
//!   hash) in SpreadsheetML, `p:modifyVerifier` (§19.2.1.19) in
//!   PresentationML (PresentationML defines no read-only recommendation, only
//!   the password to modify).
//! * Restricted editing in BlueDoc — `w:documentProtection` (§17.15.1.29)
//!   with `w:edit="readOnly"` plus the editable regions of [`crate::perm`].
//! * Sensitivity labels — properties another product wrote (`MSIP_Label_*`
//!   custom properties and the `docMetadata/LabelInfo.xml` part) are read and
//!   kept byte for byte; BlueOffice's own label goes into its own custom
//!   properties (`BO_Label_Id`, `BO_Label_Name`, `BO_Label_Marking`).
//!
//! Package-level data (custom properties, the docx settings elements) is
//! patched into the saved package only when it changed, so untouched files
//! keep those parts byte for byte. Passwords are hashed with SHA-512, a random
//! salt and 100 000 iterations ([MS-OFFCRYPTO] §2.4.2.4, `bo_crypto`).

use crate::sheet::protect::{self as prot, get, hash_attrs, parse_attrs, Attrs};
use crate::{EngineError, Result};
use bo_opc::custom::{self, CustomProperty};
use serde::{Deserialize, Serialize};

/// Custom property marking a file as final.
pub const MARK_AS_FINAL: &str = "_MarkAsFinal";
/// BlueOffice label: identifier from the administrator's list.
pub const LABEL_ID: &str = "BO_Label_Id";
/// BlueOffice label: display name.
pub const LABEL_NAME: &str = "BO_Label_Name";
/// BlueOffice label: marking text shown in the header (BlueDoc).
pub const LABEL_MARKING: &str = "BO_Label_Marking";
/// Error of an edit refused because the file is open read-only.
pub const READ_ONLY: &str = "read only";
/// Error of an edit refused by restricted editing (outside editable regions).
pub const DOC_PROTECTED: &str = "document protected";
/// Error of a password hash format BlueOffice cannot check.
pub const UNSUPPORTED_HASH: &str = "unsupported password format";
/// Error when editing needs the password to modify first.
pub const WRITE_PASSWORD_REQUIRED: &str = "write password required";

/// Part holding label metadata written by other products (kept verbatim).
const LABEL_INFO_PART: &str = "/docMetadata/LabelInfo.xml";
const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const REL_SETTINGS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings";

/// A label another product put in the file (read only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForeignLabel {
    /// Label identifier (a GUID chosen by that product).
    pub id: String,
    /// Display name, when the file carries one.
    pub name: Option<String>,
    /// The label is applied (not removed).
    pub enabled: bool,
    /// How it was applied (`Standard`, `Privileged`, …), when given.
    pub method: Option<String>,
    /// Tenant identifier, when given.
    pub site_id: Option<String>,
}

/// BlueOffice's own label, as stored in the custom properties.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelSpec {
    /// Identifier from the administrator's list (never another product's GUID).
    #[serde(default)]
    pub id: Option<String>,
    /// Display name, e.g. "Nội bộ".
    pub name: String,
    /// Marking text placed in the header (BlueDoc), when wanted.
    #[serde(default)]
    pub marking: Option<String>,
}

/// `w:documentProtection` state (§17.15.1.29).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocProtection {
    /// `w:edit` (§17.18.37): `readOnly`, `comments`, `trackedChanges`, `forms`, `none`.
    pub edit: String,
    /// `w:enforcement` is on.
    pub enforced: bool,
    /// Stopping the protection needs a password.
    pub has_password: bool,
}

/// What the UI shows (File › Info › Protect).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityInfo {
    /// `_MarkAsFinal` is set.
    pub marked_final: bool,
    /// The file recommends opening read-only.
    pub read_only_recommended: bool,
    /// A password to modify is set.
    pub write_password: bool,
    /// The session is read-only now (final, recommendation accepted, or the
    /// password to modify not given yet).
    pub read_only: bool,
    /// BlueOffice label.
    pub label: Option<LabelSpec>,
    /// Labels written by other products.
    pub foreign_labels: Vec<ForeignLabel>,
    /// Restricted editing (BlueDoc).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub protection: Option<DocProtection>,
    /// Editable regions (BlueDoc).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<crate::perm::Region>,
    /// Highlight rectangles of the regions the user may edit (when shown).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub region_rects: Vec<crate::nav::SelRect>,
    /// Regions are highlighted.
    pub show_regions: bool,
    /// Names the current user is known by (permission matching).
    pub identity: Vec<String>,
}

fn yes() -> bool {
    true
}

/// Security commands, accepted by the three engines' JSON entry points.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum SecurityCommand {
    /// Report [`SecurityInfo`].
    Security,
    /// Names of the current user (display name, e-mail) for editable regions.
    SetIdentity {
        /// Names; empty = only "everyone" regions apply.
        names: Vec<String>,
    },
    /// Open read-only (`on`) or edit; editing a write-protected file needs
    /// [`SecurityCommand::UnlockWrite`] first.
    SetReadOnly {
        /// Read-only.
        on: bool,
    },
    /// Give the password to modify; the session becomes editable when it matches.
    UnlockWrite {
        /// Password.
        password: String,
    },
    /// Mark as final (`on`) or edit anyway (removes the mark).
    MarkFinal {
        /// Final.
        on: bool,
    },
    /// Recommend opening read-only.
    SetReadOnlyRecommended {
        /// Recommend.
        on: bool,
    },
    /// Password to modify; empty removes it.
    SetWritePassword {
        /// Password.
        #[serde(default)]
        password: String,
    },
    /// Apply a BlueOffice label (`None` removes it).
    SetLabel {
        /// The label.
        #[serde(default)]
        label: Option<LabelSpec>,
    },
    /// BlueDoc: allow changes only in editable regions (`w:edit="readOnly"`).
    RestrictEditing {
        /// Password to stop the protection; empty = none.
        #[serde(default)]
        password: String,
    },
    /// BlueDoc: stop restricted editing.
    StopRestrictEditing {
        /// The protection's password (empty when it has none).
        #[serde(default)]
        password: String,
    },
    /// BlueDoc: make the selection an editable region for everyone, or for
    /// one user.
    AddEditableRegion {
        /// User (e-mail or `DOMAIN\name`); `None` = everyone.
        #[serde(default)]
        editor: Option<String>,
    },
    /// BlueDoc: remove an editable region (its text stays).
    RemoveEditableRegion {
        /// `w:id` of the region.
        id: String,
    },
    /// BlueDoc: select the next (or previous) region the user may edit.
    NextEditableRegion {
        /// Search forward.
        #[serde(default = "yes")]
        forward: bool,
    },
    /// BlueDoc: highlight the regions the user may edit.
    ShowEditableRegions {
        /// Highlight.
        on: bool,
    },
    /// BlueSheet: protected ranges of a sheet (§18.3.1.71).
    ProtectedRanges {
        /// 0-based sheet index.
        sheet: usize,
    },
    /// BlueSheet: add or replace a range users may edit on the protected sheet.
    AddProtectedRange {
        /// 0-based sheet index.
        sheet: usize,
        /// Range name.
        name: String,
        /// Cells, e.g. `B2:D9`.
        sqref: String,
        /// Password of the range; empty = none.
        #[serde(default)]
        password: String,
    },
    /// BlueSheet: remove a protected range.
    RemoveProtectedRange {
        /// 0-based sheet index.
        sheet: usize,
        /// Range name.
        name: String,
    },
    /// BlueSheet: give the password of a range for this session.
    UnlockProtectedRange {
        /// 0-based sheet index.
        sheet: usize,
        /// Range name.
        name: String,
        /// Its password.
        #[serde(default)]
        password: String,
    },
}

impl SecurityCommand {
    /// Parses a JSON command when it is a security command.
    pub fn parse(v: &serde_json::Value) -> Option<SecurityCommand> {
        serde_json::from_value(v.clone()).ok()
    }

    /// The command only reads or changes the view (allowed while read-only).
    pub fn is_view(&self) -> bool {
        matches!(
            self,
            SecurityCommand::Security
                | SecurityCommand::SetIdentity { .. }
                | SecurityCommand::SetReadOnly { .. }
                | SecurityCommand::UnlockWrite { .. }
                | SecurityCommand::NextEditableRegion { .. }
                | SecurityCommand::ShowEditableRegions { .. }
                | SecurityCommand::ProtectedRanges { .. }
                | SecurityCommand::UnlockProtectedRange { .. }
        ) || matches!(self, SecurityCommand::MarkFinal { on: false })
    }
}

/// Verifies `password` against password attributes: `algorithmName` /
/// `hashValue` / `saltValue` / `spinCount` (or PresentationML `spinValue`),
/// `cryptAlgorithmSid` + `hashData` / `saltData` (the iterated method of
/// [MS-OFFCRYPTO] §2.4.2.4) and the legacy 16-bit `legacy` attribute.
/// WordprocessingML legacy `w:hash` / `w:salt` verifiers are checked after the
/// legacy pre-processing ([`bo_crypto::protection::verify_word_legacy`]).
/// `None` when the hash is in a form BlueOffice cannot check (MD2/MD4/MD5 or a
/// non-standard `algIdExt` algorithm); `Some(true)` when no password is stored.
pub fn verify_password(attrs: &Attrs, legacy: &str, password: &str) -> Option<bool> {
    let spin = |names: &[&str]| {
        names
            .iter()
            .find_map(|n| get(attrs, n))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    };
    if let (Some(alg), Some(hash), Some(salt)) = (
        get(attrs, "algorithmName"),
        get(attrs, "hashValue"),
        get(attrs, "saltValue"),
    ) {
        return Some(bo_crypto::protection::verify_protection(
            password,
            &bo_crypto::protection::ProtectionHash {
                algorithm_name: alg.to_owned(),
                hash_value: hash.to_owned(),
                salt_value: salt.to_owned(),
                spin_count: spin(&["spinCount", "spinValue"]),
            },
        ));
    }
    if let (Some(sid), Some(hash), Some(salt)) = (
        get(attrs, "cryptAlgorithmSid"),
        get(attrs, "hashData"),
        get(attrs, "saltData"),
    ) {
        // ALG_ID numbers of the hash algorithms ([MS-OFFCRYPTO] §2.3.2).
        let alg = match sid.trim() {
            "4" => "SHA-1",
            "12" => "SHA-256",
            "13" => "SHA-384",
            "14" => "SHA-512",
            _ => return None,
        };
        return Some(bo_crypto::protection::verify_protection(
            password,
            &bo_crypto::protection::ProtectionHash {
                algorithm_name: alg.to_owned(),
                hash_value: hash.to_owned(),
                salt_value: salt.to_owned(),
                spin_count: spin(&["spinCount", "cryptSpinCount"]),
            },
        ));
    }
    // ECMA-376 Part 1, §17.15.1.29 / §17.15.1.93 legacy verifier (`w:hash` / `w:salt` /
    // `w:cryptAlgorithmSid` / `w:cryptSpinCount`), hashed after the legacy pre-processing.
    if let Some(hash) = get(attrs, "hash") {
        // A non-standard algorithm (`algIdExt`) cannot be checked.
        if get(attrs, "algIdExt").is_some() {
            return None;
        }
        let alg = bo_crypto::protection::word_legacy_algorithm(get(attrs, "cryptAlgorithmSid"))?;
        return Some(bo_crypto::protection::verify_word_legacy(
            password,
            &bo_crypto::protection::WordLegacyHash {
                algorithm: alg,
                hash: hash.to_owned(),
                salt: get(attrs, "salt").unwrap_or_default().to_owned(),
                spin_count: spin(&["cryptSpinCount", "spinCount"]),
            },
        ));
    }
    match get(attrs, legacy) {
        Some(hex) => Some(
            u16::from_str_radix(hex.trim(), 16)
                .is_ok_and(|h| h == bo_crypto::protection::legacy_password_hash(password)),
        ),
        None => Some(true),
    }
}

/// Whether the attributes store a password.
pub fn has_password(attrs: &Attrs) -> bool {
    [
        "hashValue",
        "hashData",
        "hash",
        "password",
        "reservationPassword",
    ]
    .iter()
    .any(|n| get(attrs, n).is_some())
}

/// Hash attributes for a new password with a namespace prefix (`w:`), empty
/// for no password.
fn prefixed_hash(prefix: &str, password: &str) -> Result<Attrs> {
    Ok(hash_attrs("", password)?
        .into_iter()
        .map(|(k, v)| (format!("{prefix}{k}"), v))
        .collect())
}

fn is_hash_attr(name: &str) -> bool {
    matches!(
        prot::local(name),
        "algorithmName"
            | "hashValue"
            | "saltValue"
            | "spinCount"
            | "spinValue"
            | "cryptProviderType"
            | "cryptAlgorithmClass"
            | "cryptAlgorithmType"
            | "cryptAlgorithmSid"
            | "cryptSpinCount"
            | "cryptProvider"
            | "algIdExt"
            | "algIdExtSource"
            | "cryptProviderTypeExt"
            | "cryptProviderTypeExtSource"
            | "hash"
            | "salt"
            | "hashData"
            | "saltData"
    )
}

/// Security state of the open file (package-level parts and the session).
#[derive(Debug, Clone, Default)]
pub(crate) struct FileSecurity {
    /// Custom properties as read.
    original: Vec<CustomProperty>,
    /// Custom properties wanted on save.
    pub custom: Vec<CustomProperty>,
    /// Labels of other products (read only).
    foreign: Vec<ForeignLabel>,
    /// docx `w:writeProtection` attributes as read / as wanted.
    orig_write: Option<Attrs>,
    pub write: Option<Attrs>,
    /// docx `w:documentProtection` attributes as read / as wanted.
    orig_protect: Option<Attrs>,
    pub protect: Option<Attrs>,
    /// State at the last save (`modified`).
    saved: SavedState,
    /// The session is read-only.
    pub read_only: bool,
    /// The password to modify was given.
    pub write_unlocked: bool,
    /// Editable regions are highlighted.
    pub show_regions: bool,
    /// A model-level protection setting changed (presentation password to modify).
    pub model_changed: bool,
    /// Fingerprint of the protected content at a session version.
    pub skeleton: Option<(u64, u64)>,
}

type SavedState = (Vec<CustomProperty>, Option<Attrs>, Option<Attrs>);

/// Attributes of the first `local` element child of a settings part.
fn settings_element(xml: &str, local: &str) -> Option<Attrs> {
    find_element(xml, local).map(|(a, b)| parse_attrs(&xml[a..b]))
}

/// Byte span of the first element named `local` (any prefix) in `xml`.
fn find_element(xml: &str, local: &str) -> Option<(usize, usize)> {
    let mut i = 0;
    while let Some(k) = xml[i..].find('<') {
        let start = i + k;
        let rest = &xml[start + 1..];
        let end_name = rest
            .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .unwrap_or(rest.len());
        let qname = &rest[..end_name];
        if prot::local(qname) == local && !qname.starts_with('/') {
            let gt = rest.find('>')?;
            let tag_end = start + 1 + gt + 1;
            if rest[..gt].ends_with('/') {
                return Some((start, tag_end));
            }
            let close = format!("</{qname}>");
            let c = xml[tag_end..].find(&close)?;
            return Some((start, tag_end + c + close.len()));
        }
        i = start + 1;
    }
    None
}

/// CT_Settings children after `w:documentProtection` (§17.15.1.78).
const AFTER_DOC_PROTECTION: &[&str] = &[
    "autoFormatOverride",
    "styleLockTheme",
    "styleLockQFSet",
    "defaultTabStop",
    "autoHyphenation",
    "consecutiveHyphenLimit",
    "hyphenationZone",
    "doNotHyphenateCaps",
    "showEnvelope",
    "summaryLength",
    "clickAndTypeStyle",
    "defaultTableStyle",
    "evenAndOddHeaders",
    "bookFoldRevPrinting",
    "bookFoldPrinting",
    "bookFoldPrintingSheets",
    "drawingGridHorizontalSpacing",
    "drawingGridVerticalSpacing",
    "displayHorizontalDrawingGridEvery",
    "displayVerticalDrawingGridEvery",
    "doNotUseMarginsForDrawingGridOrigin",
    "drawingGridHorizontalOrigin",
    "drawingGridVerticalOrigin",
    "doNotShadeFormData",
    "noPunctuationKerning",
    "characterSpacingControl",
    "printTwoOnOne",
    "strictFirstAndLastChars",
    "noLineBreaksAfter",
    "noLineBreaksBefore",
    "savePreviewPicture",
    "doNotValidateAgainstSchema",
    "saveInvalidXml",
    "ignoreMixedContent",
    "alwaysShowPlaceholderText",
    "doNotDemarcateInvalidXml",
    "saveXmlDataOnly",
    "useXSLTWhenSaving",
    "saveThroughXslt",
    "showXMLTags",
    "alwaysMergeEmptyNamespace",
    "updateFields",
    "hdrShapeDefaults",
    "footnotePr",
    "endnotePr",
    "compat",
    "docVars",
    "rsids",
    "mathPr",
    "attachedSchema",
    "themeFontLang",
    "clrSchemeMapping",
    "doNotIncludeSubdocsInStats",
    "doNotAutoCompressPictures",
    "forceUpgrade",
    "captions",
    "readModeInkLockDown",
    "smartTagType",
    "schemaLibrary",
    "shapeDefaults",
    "doNotEmbedSmartTags",
    "decimalSymbol",
    "listSeparator",
];

/// Replaces (or removes, `el = None`) the settings child `local`. A new element
/// goes first (`first`) or before the first of `followers`.
fn patch_settings(xml: &str, local: &str, el: Option<&str>, followers: &[&str]) -> String {
    let mut s = xml.to_owned();
    if let Some((a, b)) = find_element(&s, local) {
        s.replace_range(a..b, "");
    }
    let Some(el) = el else {
        return s;
    };
    let at = if followers.is_empty() {
        // Right after the `<w:settings …>` start tag.
        find_element_start(&s, "settings").and_then(|i| s[i..].find('>').map(|g| i + g + 1))
    } else {
        followers
            .iter()
            .filter_map(|n| find_element(&s, n).map(|(a, _)| a))
            .min()
            .or_else(|| s.rfind("</"))
    };
    if let Some(at) = at {
        s.insert_str(at, el);
    }
    s
}

fn find_element_start(xml: &str, local: &str) -> Option<usize> {
    let mut i = 0;
    while let Some(k) = xml[i..].find('<') {
        let start = i + k;
        let rest = &xml[start + 1..];
        let end = rest
            .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .unwrap_or(rest.len());
        if prot::local(&rest[..end]) == local {
            return Some(start);
        }
        i = start + 1;
    }
    None
}

/// Prefix of the WordprocessingML namespace in a settings part (`w`).
fn w_prefix(xml: &str) -> String {
    let decl = format!("=\"{W_NS}\"");
    xml.find(&decl)
        .and_then(|i| {
            let head = &xml[..i];
            head.rfind("xmlns:").map(|j| head[j + 6..].to_owned())
        })
        .unwrap_or_else(|| "w".into())
}

fn element_xml(qname: &str, attrs: &Attrs) -> String {
    prot::to_xml(qname, attrs)
}

/// Settings part of a docx package.
fn settings_part(pkg: &bo_opc::Package) -> Option<String> {
    let main = pkg
        .related_part("/", bo_opc::REL_OFFICE_DOCUMENT)
        .ok()
        .flatten()?;
    pkg.related_part(&main, REL_SETTINGS).ok().flatten()
}

/// Labels written by other products: `MSIP_Label_<id>_<field>` custom
/// properties and `clbl:label` elements of the label metadata part.
fn foreign_labels(pkg: &bo_opc::Package, props: &[CustomProperty]) -> Vec<ForeignLabel> {
    let mut out: Vec<ForeignLabel> = Vec::new();
    for p in props {
        let Some(rest) = p.name.strip_prefix("MSIP_Label_") else {
            continue;
        };
        let Some((id, field)) = rest.rsplit_once('_') else {
            continue;
        };
        let i = if let Some(i) = out.iter().position(|l| l.id.eq_ignore_ascii_case(id)) {
            i
        } else {
            out.push(ForeignLabel {
                id: id.to_owned(),
                name: None,
                enabled: false,
                method: None,
                site_id: None,
            });
            out.len() - 1
        };
        let l = &mut out[i];
        match field {
            "Enabled" => l.enabled = p.as_bool().unwrap_or(false),
            "Name" => l.name = Some(p.value.clone()),
            "Method" => l.method = Some(p.value.clone()),
            "SiteId" => l.site_id = Some(p.value.clone()),
            _ => {}
        }
    }
    if let Some(part) = pkg.part(LABEL_INFO_PART) {
        let xml = String::from_utf8_lossy(&part.data);
        let mut i = 0;
        while let Some((a, b)) = find_element(&xml[i..], "label") {
            let attrs = parse_attrs(&xml[i + a..i + b]);
            let id = get(&attrs, "id").unwrap_or_default().to_owned();
            if !id.is_empty() && !out.iter().any(|l| l.id.eq_ignore_ascii_case(&id)) {
                out.push(ForeignLabel {
                    enabled: get(&attrs, "enabled").is_some_and(|v| v == "1" || v == "true")
                        && !get(&attrs, "removed").is_some_and(|v| v == "1" || v == "true"),
                    method: get(&attrs, "method").map(str::to_owned),
                    site_id: get(&attrs, "siteId").map(str::to_owned),
                    name: None,
                    id,
                });
            }
            i += b;
        }
    }
    out
}

impl FileSecurity {
    /// Reads the security metadata of an OOXML package (anything else: none).
    pub fn read(bytes: &[u8]) -> FileSecurity {
        let Ok(pkg) = bo_opc::Package::read(bytes, &bo_opc::Limits::default()) else {
            return FileSecurity::default();
        };
        let custom = custom::read_custom_properties(&pkg).unwrap_or_default();
        let foreign = foreign_labels(&pkg, &custom);
        let settings = settings_part(&pkg)
            .and_then(|n| pkg.part(&n))
            .map(|p| String::from_utf8_lossy(&p.data).into_owned());
        let (write, protect) = settings.map_or((None, None), |x| {
            (
                settings_element(&x, "writeProtection"),
                settings_element(&x, "documentProtection"),
            )
        });
        FileSecurity {
            original: custom.clone(),
            saved: (custom.clone(), write.clone(), protect.clone()),
            custom,
            foreign,
            orig_write: write.clone(),
            write,
            orig_protect: protect.clone(),
            protect,
            ..FileSecurity::default()
        }
    }

    /// A custom property by name (case-insensitive).
    pub fn prop(&self, name: &str) -> Option<&CustomProperty> {
        self.custom
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// Sets or removes a custom property.
    pub fn set_prop(&mut self, name: &str, value: Option<CustomProperty>) {
        let i = self
            .custom
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(name));
        match (i, value) {
            (Some(i), Some(v)) => self.custom[i] = v,
            (Some(i), None) => {
                self.custom.remove(i);
            }
            (None, Some(v)) => self.custom.push(v),
            (None, None) => {}
        }
    }

    /// `_MarkAsFinal` is true.
    pub fn marked_final(&self) -> bool {
        self.prop(MARK_AS_FINAL)
            .and_then(CustomProperty::as_bool)
            .unwrap_or(false)
    }

    /// BlueOffice label.
    pub fn label(&self) -> Option<LabelSpec> {
        let name = self.prop(LABEL_NAME)?.value.clone();
        Some(LabelSpec {
            id: self.prop(LABEL_ID).map(|p| p.value.clone()),
            name,
            marking: self
                .prop(LABEL_MARKING)
                .map(|p| p.value.clone())
                .filter(|m| !m.is_empty()),
        })
    }

    /// Sets the BlueOffice label properties (`None` removes them).
    pub fn set_label(&mut self, label: Option<&LabelSpec>) {
        let text = |n: &str, v: Option<&str>| v.map(|v| CustomProperty::text(n, v));
        self.set_prop(
            LABEL_ID,
            text(LABEL_ID, label.and_then(|l| l.id.as_deref())),
        );
        self.set_prop(LABEL_NAME, text(LABEL_NAME, label.map(|l| l.name.as_str())));
        self.set_prop(
            LABEL_MARKING,
            text(
                LABEL_MARKING,
                label
                    .and_then(|l| l.marking.as_deref())
                    .filter(|m| !m.is_empty()),
            ),
        );
    }

    /// Labels of other products.
    pub fn foreign(&self) -> &[ForeignLabel] {
        &self.foreign
    }

    /// Changed since the last save.
    pub fn dirty(&self) -> bool {
        self.model_changed
            || self.saved.0 != self.custom
            || self.saved.1 != self.write
            || self.saved.2 != self.protect
    }

    /// The current state was saved.
    pub fn mark_saved(&mut self) {
        self.saved = (
            self.custom.clone(),
            self.write.clone(),
            self.protect.clone(),
        );
        self.model_changed = false;
    }

    /// docx: `w:writeProtection/@w:recommended`.
    pub fn docx_recommended(&self) -> bool {
        self.write
            .as_ref()
            .is_some_and(|a| prot::flag(a, "recommended", false))
    }

    /// docx: a password to modify is set.
    pub fn docx_write_password(&self) -> bool {
        self.write.as_ref().is_some_and(has_password)
    }

    /// docx: sets `w:recommended` and/or the password to modify.
    pub fn set_docx_write(
        &mut self,
        recommended: Option<bool>,
        password: Option<&str>,
    ) -> Result<()> {
        let mut a = self.write.clone().unwrap_or_default();
        if let Some(pw) = password {
            a.retain(|(k, _)| !is_hash_attr(k));
            a.extend(prefixed_hash("w:", pw)?);
        }
        if let Some(on) = recommended {
            a.retain(|(k, _)| prot::local(k) != "recommended");
            if on {
                a.insert(0, ("w:recommended".into(), "1".into()));
            }
        }
        let keep = a
            .iter()
            .any(|(k, _)| prot::local(k) == "recommended" || is_hash_attr(k));
        self.write = keep.then_some(a);
        Ok(())
    }

    /// docx: restricted editing state.
    pub fn doc_protection(&self) -> Option<DocProtection> {
        let a = self.protect.as_ref()?;
        Some(DocProtection {
            edit: get(a, "edit").unwrap_or("none").to_owned(),
            enforced: prot::flag(a, "enforcement", false),
            has_password: has_password(a),
        })
    }

    /// docx: only editable regions may change (`readOnly`, enforced).
    pub fn regions_enforced(&self) -> bool {
        self.doc_protection()
            .is_some_and(|p| p.enforced && p.edit == "readOnly")
    }

    /// docx: starts restricted editing.
    pub fn protect_doc(&mut self, password: &str) -> Result<()> {
        let mut a: Attrs = vec![
            ("w:edit".into(), "readOnly".into()),
            ("w:enforcement".into(), "1".into()),
        ];
        a.extend(prefixed_hash("w:", password)?);
        self.protect = Some(a);
        self.skeleton = None;
        Ok(())
    }

    /// docx: stops restricted editing after checking the password.
    pub fn unprotect_doc(&mut self, password: &str) -> Result<()> {
        let Some(a) = &self.protect else {
            return Ok(());
        };
        match verify_password(a, "", password) {
            Some(true) => {
                self.protect = None;
                self.skeleton = None;
                Ok(())
            }
            Some(false) => Err(EngineError::BadCommand(
                prot::WRONG_PROTECTION_PASSWORD.into(),
            )),
            None => Err(EngineError::BadCommand(UNSUPPORTED_HASH.into())),
        }
    }

    /// Writes the changed metadata into saved package bytes (unchanged: as is).
    pub fn apply(&self, bytes: Vec<u8>) -> Result<Vec<u8>> {
        let props_changed = self.custom != self.original;
        let settings_changed = self.write != self.orig_write || self.protect != self.orig_protect;
        if !props_changed && !settings_changed {
            return Ok(bytes);
        }
        let opc = |e: bo_opc::OpcError| EngineError::Format(e.to_string());
        let mut pkg = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).map_err(opc)?;
        if props_changed {
            let mut changes: Vec<(String, Option<CustomProperty>)> = Vec::new();
            for p in &self.custom {
                let before = self
                    .original
                    .iter()
                    .find(|o| o.name.eq_ignore_ascii_case(&p.name));
                if before != Some(p) {
                    changes.push((p.name.clone(), Some(p.clone())));
                }
            }
            for o in &self.original {
                if self.prop(&o.name).is_none() {
                    changes.push((o.name.clone(), None));
                }
            }
            custom::update_custom_properties(&mut pkg, &changes).map_err(opc)?;
        }
        if settings_changed {
            if let Some(name) = settings_part(&pkg) {
                if let Some(part) = pkg.part(&name) {
                    let mut xml = String::from_utf8_lossy(&part.data).into_owned();
                    let p = w_prefix(&xml);
                    let qualify = |a: &Attrs| -> Attrs {
                        a.iter()
                            .map(|(k, v)| {
                                let k = if k.contains(':') {
                                    format!("{p}:{}", prot::local(k))
                                } else {
                                    k.clone()
                                };
                                (k, v.clone())
                            })
                            .collect()
                    };
                    if self.write != self.orig_write {
                        // §17.15.1.93 writeProtection is the first child of CT_Settings.
                        let el = self
                            .write
                            .as_ref()
                            .map(|a| element_xml(&format!("{p}:writeProtection"), &qualify(a)));
                        xml = patch_settings(&xml, "writeProtection", el.as_deref(), &[]);
                    }
                    if self.protect != self.orig_protect {
                        // §17.15.1.29 documentProtection.
                        let el = self
                            .protect
                            .as_ref()
                            .map(|a| element_xml(&format!("{p}:documentProtection"), &qualify(a)));
                        xml = patch_settings(
                            &xml,
                            "documentProtection",
                            el.as_deref(),
                            AFTER_DOC_PROTECTION,
                        );
                    }
                    pkg.put_part(&name, xml.into_bytes());
                }
            }
        }
        pkg.write().map_err(opc)
    }

    /// Error message of a refused edit.
    pub fn refusal(&self) -> &'static str {
        if self.read_only {
            READ_ONLY
        } else {
            DOC_PROTECTED
        }
    }

    /// The file asks to be opened read-only (final, recommendation, password to modify).
    pub fn opens_read_only(&self, recommended: bool, write_password: bool) -> bool {
        self.marked_final() || recommended || write_password
    }
}

// ------------------------------------------------------------ PresentationML

/// `p:modifyVerifier` (§19.2.1.19) attributes of a presentation.
pub(crate) fn pptx_verifier(pres: &bo_model::slide::Presentation) -> Option<Attrs> {
    pres.extra
        .iter()
        .find(|e| prot::local(&e.xml.name) == "modifyVerifier")
        .map(|e| e.xml.attrs.clone())
}

/// Sets the password to modify of a presentation (empty removes it).
pub(crate) fn set_pptx_verifier(
    pres: &mut bo_model::slide::Presentation,
    password: &str,
) -> Result<()> {
    pres.extra
        .retain(|e| prot::local(&e.xml.name) != "modifyVerifier");
    if password.is_empty() {
        return Ok(());
    }
    // CT_ModifyVerifier uses `spinValue` for the iteration count.
    let attrs = hash_attrs("", password)?
        .into_iter()
        .map(|(k, v)| {
            if k == "spinCount" {
                ("spinValue".to_owned(), v)
            } else {
                (k, v)
            }
        })
        .collect();
    let el = bo_model::slide::XmlElement {
        name: "p:modifyVerifier".into(),
        attrs,
        children: Vec::new(),
    };
    pres.extra.push(bo_pptx::presentation_extra(el));
    pres.extra.sort_by_key(|e| e.key);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_elements_are_placed_by_schema_order() {
        let x = r#"<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:zoom w:percent="100"/><w:defaultTabStop w:val="720"/><w:compat/></w:settings>"#;
        let a = patch_settings(
            x,
            "writeProtection",
            Some(r#"<w:writeProtection w:recommended="1"/>"#),
            &[],
        );
        assert!(a.contains(r#"main"><w:writeProtection w:recommended="1"/><w:zoom"#));
        let b = patch_settings(
            &a,
            "documentProtection",
            Some(r#"<w:documentProtection w:edit="readOnly" w:enforcement="1"/>"#),
            AFTER_DOC_PROTECTION,
        );
        assert!(b.contains(r#"<w:zoom w:percent="100"/><w:documentProtection w:edit="readOnly" w:enforcement="1"/><w:defaultTabStop"#));
        let c = patch_settings(&b, "writeProtection", None, &[]);
        assert!(!c.contains("writeProtection"));
        assert_eq!(w_prefix(x), "w");
        assert_eq!(
            settings_element(&b, "documentProtection").map(|a| a.len()),
            Some(2)
        );
    }

    #[test]
    fn password_forms() {
        let h = prefixed_hash("w:", "mở").unwrap();
        assert_eq!(verify_password(&h, "", "mở"), Some(true));
        assert_eq!(verify_password(&h, "", "mo"), Some(false));
        let legacy = parse_attrs(r#"<fileSharing reservationPassword="CBEB"/>"#);
        assert_eq!(
            verify_password(&legacy, "reservationPassword", "test"),
            Some(true)
        );
        // A legacy verifier that does not match is now a wrong password, not "unsupported".
        let word = parse_attrs(r#"<w:writeProtection w:hash="AAAA" w:salt="BBBB"/>"#);
        assert_eq!(verify_password(&word, "", "x"), Some(false));
        let md5 = parse_attrs(r#"<w:writeProtection w:cryptAlgorithmSid="3" w:hash="AAAA"/>"#);
        assert_eq!(verify_password(&md5, "", "x"), None);
        assert_eq!(verify_password(&Vec::new(), "", "x"), Some(true));
    }

    /// ECMA-376 Part 1, §17.15.1.93 writeProtection / §17.15.1.29 documentProtection with the
    /// legacy attributes: the hash is computed here per the standard (key of "Example" →
    /// "7EEDCE64", SHA-1, salt prepended, 100 000 spins).
    #[test]
    fn word_legacy_verifier_is_checked() {
        let salt = [7u8, 1, 9, 9, 2, 0, 2, 6, 1, 1, 1, 2, 3, 5, 8, 13];
        let h = bo_crypto::protection::word_legacy_hash_with_salt(
            bo_crypto::HashAlgorithm::Sha1,
            "Example",
            &salt,
            100_000,
        );
        let xml = format!(
            r#"<w:documentProtection w:edit="forms" w:enforcement="1" w:cryptProviderType="rsaFull" w:cryptAlgorithmClass="hash" w:cryptAlgorithmType="typeAny" w:cryptAlgorithmSid="4" w:cryptSpinCount="100000" w:hash="{}" w:salt="{}"/>"#,
            h.hash, h.salt
        );
        let a = parse_attrs(&xml);
        assert_eq!(verify_password(&a, "", "Example"), Some(true));
        assert_eq!(verify_password(&a, "", "example"), Some(false));
        assert_eq!(verify_password(&a, "", ""), Some(false));
    }
}
