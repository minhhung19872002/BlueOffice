//! Read-only view of the VBA macro source of the open file (CORE-VBA-002).
//!
//! BlueOffice never runs VBA. The project of a macro-enabled file is kept byte for
//! byte when saving ([`crate::variant`]); this module only *reads* the source text of
//! its modules ([MS-OVBA], `bo_binary::vba`) so that the user can look at it and
//! turn a module into a BlueScript draft (the converter of `packages/script`).
//!
//! The command `{"cmd":"vbaProject"}` is answered by the three JSON bridges
//! ([`crate::Engine::command_json`], [`crate::Engine::slide_command_json`],
//! [`crate::sheet::SheetEngine::exec_json`]) with a [`VbaProjectOut`]. It changes
//! nothing, so it is allowed in read-only and protected documents.

use serde::Serialize;

/// `cmd` name of the command.
pub const COMMAND: &str = "vbaProject";

/// One module of the project.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VbaModuleOut {
    /// Module name.
    pub name: String,
    /// `standard`, `document`, `class` or `form`.
    pub kind: &'static str,
    /// Source text with `\n` line ends, without the leading `Attribute VB_…` lines.
    pub source: String,
    /// The module could not be read (missing or damaged stream, size limit).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub unreadable: bool,
}

/// The VBA project of the open file, for display only.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VbaProjectOut {
    /// Project name; empty when the file has no VBA project.
    pub name: String,
    /// Modules in the order of the project.
    pub modules: Vec<VbaModuleOut>,
    /// Code page of the project's text ([MS-OVBA] PROJECTCODEPAGE).
    pub code_page: u16,
    /// The code page is one BlueOffice decodes (1252, 1258, UTF-8); otherwise the
    /// text was read as Windows-1252 and accented characters may be wrong.
    pub code_page_supported: bool,
    /// Modules or sources were left out because of the size limits.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub truncated: bool,
    /// The project is present but could not be read at all.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The source as shown: `\n` line ends, and without the `Attribute VB_…` lines a
/// module stream starts with (module attributes are not part of the code the
/// author typed; they would only confuse the converter).
pub fn display_source(raw: &str) -> String {
    let text = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut rest = text.as_str();
    while rest
        .get(..13)
        .is_some_and(|h| h.eq_ignore_ascii_case("Attribute VB_"))
    {
        rest = rest.split_once('\n').map_or("", |(_, r)| r);
    }
    rest.to_owned()
}

impl From<bo_binary::VbaProject> for VbaProjectOut {
    fn from(p: bo_binary::VbaProject) -> Self {
        VbaProjectOut {
            name: p.name,
            modules: p
                .modules
                .into_iter()
                .map(|m| VbaModuleOut {
                    name: m.name,
                    kind: m.kind.as_str(),
                    source: display_source(&m.source),
                    unreadable: m.unreadable,
                })
                .collect(),
            code_page: p.code_page,
            code_page_supported: p.code_page_supported,
            truncated: p.truncated,
            error: None,
        }
    }
}

/// The VBA project of a plain (decrypted) file in any supported container;
/// `None` when it has none. A project that cannot be parsed is reported with
/// [`VbaProjectOut::error`] rather than hidden.
pub fn read(bytes: &[u8]) -> Option<VbaProjectOut> {
    match bo_binary::read_vba_from_file(bytes) {
        Ok(Some(p)) => Some(p.into()),
        Ok(None) => None,
        Err(e) => Some(VbaProjectOut {
            code_page_supported: true,
            error: Some(e.to_string()),
            ..VbaProjectOut::default()
        }),
    }
}

/// [`read`] for a file as stored: decrypts an encrypted package with `password`
/// ([MS-OFFCRYPTO]) and unwraps a Flat OPC file first.
pub fn read_file(bytes: &[u8], password: Option<&str>) -> crate::Result<Option<VbaProjectOut>> {
    let plain = crate::decrypt_package(bytes, password)?;
    let bytes = plain.as_deref().unwrap_or(bytes);
    let flat = crate::web_formats::flat_package(bytes)?;
    Ok(read(flat.as_deref().unwrap_or(bytes)))
}

/// What the engines keep of the open file: the project when `macros` were detected.
pub(crate) fn of_open_file(macros: bool, bytes: &[u8]) -> Option<Box<VbaProjectOut>> {
    if macros {
        read(bytes).map(Box::new)
    } else {
        None
    }
}

/// `v` is the `vbaProject` command.
pub(crate) fn is_command(v: &serde_json::Value) -> bool {
    v.get("cmd").and_then(serde_json::Value::as_str) == Some(COMMAND)
}

/// The answer of the command for the stored project (an empty project when the
/// file has none).
pub(crate) fn answer(vba: Option<&VbaProjectOut>) -> String {
    let empty = VbaProjectOut {
        code_page_supported: true,
        ..VbaProjectOut::default()
    };
    serde_json::to_string(vba.unwrap_or(&empty)).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_source_drops_attribute_header_only() {
        let raw = "Attribute VB_Name = \"M\"\r\nattribute vb_Exposed = False\r\nOption Explicit\r\nSub A()\r\nAttribute A.VB_Description = \"x\"\r\nEnd Sub\r\n";
        assert_eq!(
            display_source(raw),
            "Option Explicit\nSub A()\nAttribute A.VB_Description = \"x\"\nEnd Sub\n"
        );
        assert_eq!(display_source("Attribute VB_Name = \"M\""), "");
        assert_eq!(display_source("Sub ế()\rEnd Sub"), "Sub ế()\nEnd Sub");
        assert_eq!(display_source(""), "");
    }
}
