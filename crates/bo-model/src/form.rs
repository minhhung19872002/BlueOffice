//! Legacy form fields (ECMA-376 Part 1, §17.16.17 ffData) and document protection
//! (§17.15.1.29 documentProtection).
//!
//! A form field is a complex field `FORMTEXT`, `FORMCHECKBOX` or `FORMDROPDOWN` whose begin
//! character carries `w:ffData`. In a paragraph it is a [`crate::RunContent::FormField`] marker
//! (the begin character, the instruction and the separator) followed by the result content and a
//! [`crate::RunContent::FormEnd`] marker (the end character). The result of a text field is
//! ordinary text; check boxes and drop-down lists show their state instead of the result.

use serde::{Deserialize, Serialize};

/// Shown by an empty text field (five en spaces, as the classic format writes it).
pub const EMPTY_TEXT_RESULT: &str = "\u{2002}\u{2002}\u{2002}\u{2002}\u{2002}";

/// Help or status text of a form field (§17.16.21 helpText, §17.16.31 statusText).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FormHelpText {
    /// `w:type="autoText"`: `text` names an AutoText entry (else it is the text itself).
    pub auto_text: bool,
    /// `w:val`.
    pub text: String,
}

/// §17.18 `ST_FFTextType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextInputType {
    /// `regular`: any text.
    #[default]
    Regular,
    /// `number`.
    Number,
    /// `date`.
    Date,
    /// `currentDate`: the date when the field is calculated.
    CurrentDate,
    /// `currentTime`: the time when the field is calculated.
    CurrentTime,
    /// `calculated`: the result of the `=` expression in `default`.
    Calculated,
}

impl TextInputType {
    /// Parse an `ST_FFTextType` value.
    #[must_use]
    pub fn parse(s: &str) -> TextInputType {
        match s {
            "number" => TextInputType::Number,
            "date" => TextInputType::Date,
            "currentDate" => TextInputType::CurrentDate,
            "currentTime" => TextInputType::CurrentTime,
            "calculated" => TextInputType::Calculated,
            _ => TextInputType::Regular,
        }
    }

    /// The `ST_FFTextType` value.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            TextInputType::Regular => "regular",
            TextInputType::Number => "number",
            TextInputType::Date => "date",
            TextInputType::CurrentDate => "currentDate",
            TextInputType::CurrentTime => "currentTime",
            TextInputType::Calculated => "calculated",
        }
    }
}

/// §17.16.33 textInput.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FormTextInput {
    /// §17.16.34 type.
    #[serde(rename = "type")]
    pub input_type: TextInputType,
    /// §17.16.11 default (the default text, or the expression of a calculated field).
    pub default: String,
    /// §17.16.26 maxLength (0 = no limit).
    pub max_length: u32,
    /// §17.16.20 format: a case (`UPPERCASE`, `LOWERCASE`, `FIRST CAPITAL`, `TITLE CASE`), a
    /// numeric picture (§17.16.4.2) or a date picture (§17.16.4.1).
    pub format: String,
}

/// §17.16.7 checkBox.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FormCheckBox {
    /// §17.16.29 size in half-points; `None` = §17.16.30 sizeAuto (the size of the text).
    pub size: Option<u32>,
    /// §17.16.10 default state.
    pub default: bool,
    /// §17.16.8 checked: the current state (`None` = the default state).
    pub checked: Option<bool>,
}

impl FormCheckBox {
    /// Whether the box is checked now.
    #[must_use]
    pub fn is_checked(&self) -> bool {
        self.checked.unwrap_or(self.default)
    }
}

/// §17.16.9 ddList.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FormDropDown {
    /// §17.16.28 result: index of the selected entry (`None` = the default entry).
    pub result: Option<u32>,
    /// §17.16.12 default entry index.
    pub default: Option<u32>,
    /// §17.16.25 listEntry values.
    pub entries: Vec<String>,
}

impl FormDropDown {
    /// Index of the selected entry.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.result.or(self.default).unwrap_or(0) as usize
    }
}

/// What a form field holds.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FormFieldKind {
    /// `FORMTEXT` with §17.16.33 textInput.
    Text(FormTextInput),
    /// `FORMCHECKBOX` with §17.16.7 checkBox.
    CheckBox(FormCheckBox),
    /// `FORMDROPDOWN` with §17.16.9 ddList.
    DropDown(FormDropDown),
}

impl FormFieldKind {
    /// The field instruction keyword (§17.16.5.20 / .19 / .21).
    #[must_use]
    pub fn keyword(&self) -> &'static str {
        match self {
            FormFieldKind::Text(_) => "FORMTEXT",
            FormFieldKind::CheckBox(_) => "FORMCHECKBOX",
            FormFieldKind::DropDown(_) => "FORMDROPDOWN",
        }
    }
}

/// A legacy form field: the parsed §17.16.17 ffData of its begin character.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormField {
    /// §17.16.27 name (also the name of the bookmark around the field).
    pub name: Option<String>,
    /// §17.16.14 enabled: the field can be filled in.
    pub enabled: bool,
    /// §17.16.6 calcOnExit: fields are recalculated when the user leaves this one.
    #[serde(default)]
    pub calc_on_exit: bool,
    /// §17.16.15 entryMacro (kept, never run).
    pub entry_macro: Option<String>,
    /// §17.16.16 exitMacro (kept, never run).
    pub exit_macro: Option<String>,
    /// §17.16.21 helpText.
    pub help_text: Option<FormHelpText>,
    /// §17.16.31 statusText.
    pub status_text: Option<FormHelpText>,
    /// The field type and its data.
    #[serde(flatten)]
    pub kind: FormFieldKind,
    /// Other ffData children kept verbatim (`w:label`, `w:tabIndex`, …).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unknown: Vec<String>,
    /// Instruction text as read (`" FORMTEXT "`); empty = written from the kind.
    #[serde(default)]
    pub instr: String,
    /// A separator character follows the instruction (always for text fields).
    #[serde(default)]
    pub separate: bool,
    /// §17.16.18 fldLock on the begin character.
    #[serde(default)]
    pub locked: bool,
    /// `w:ffData` as read (written back while the parsed fields are unchanged).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ff_xml: Option<String>,
}

impl FormField {
    /// A new, enabled field of `kind`.
    #[must_use]
    pub fn new(kind: FormFieldKind, name: Option<String>) -> FormField {
        FormField {
            name,
            enabled: true,
            calc_on_exit: false,
            entry_macro: None,
            exit_macro: None,
            help_text: None,
            status_text: None,
            kind,
            unknown: Vec::new(),
            instr: String::new(),
            separate: true,
            locked: false,
            ff_xml: None,
        }
    }

    /// A text field.
    #[must_use]
    pub fn is_text(&self) -> bool {
        matches!(self.kind, FormFieldKind::Text(_))
    }

    /// What a check box or drop-down list shows (☒ / ☐, the selected entry); `None` for a
    /// text field, whose result is ordinary text.
    #[must_use]
    pub fn display(&self) -> Option<String> {
        match &self.kind {
            FormFieldKind::Text(_) => None,
            FormFieldKind::CheckBox(c) => Some(
                if c.is_checked() {
                    "\u{2612}"
                } else {
                    "\u{2610}"
                }
                .to_owned(),
            ),
            FormFieldKind::DropDown(d) => Some(
                d.entries
                    .get(d.selected())
                    .cloned()
                    .unwrap_or_else(|| EMPTY_TEXT_RESULT.to_owned()),
            ),
        }
    }

    /// The parsed data only (instruction text, separator and source XML cleared): two fields
    /// with equal data write the same ffData.
    #[must_use]
    pub fn data(&self) -> FormField {
        FormField {
            instr: String::new(),
            separate: false,
            locked: false,
            ff_xml: None,
            ..self.clone()
        }
    }
}

/// §17.18 `ST_DocProtect`.
pub const PROTECT_FORMS: &str = "forms";

/// Document editing restrictions (§17.15.1.29 documentProtection in settings).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocProtection {
    /// `w:edit` (§17.18 `ST_DocProtect`: `none`, `readOnly`, `comments`, `trackedChanges`, `forms`).
    pub edit: String,
    /// `w:enforcement`: the restriction is on.
    pub enforced: bool,
    /// `w:formatting`: formatting is restricted too.
    pub formatting: bool,
    /// Password verifier and other attributes (`w:algorithmName`, `w:hashValue`, `w:saltValue`,
    /// `w:spinCount`, `w:cryptProviderType`, `w:hash`, `w:salt`, …) as (local name, value).
    pub attrs: Vec<(String, String)>,
}

impl DocProtection {
    /// Value of attribute `name`.
    #[must_use]
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Filling in forms is enforced.
    #[must_use]
    pub fn forms_enforced(&self) -> bool {
        self.enforced && self.edit == PROTECT_FORMS
    }

    /// A password verifier is present.
    #[must_use]
    pub fn has_password(&self) -> bool {
        self.attr("hashValue").is_some() || self.attr("hash").is_some()
    }
}
