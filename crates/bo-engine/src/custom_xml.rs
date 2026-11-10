//! BlueDoc custom XML parts and content control data binding (ECMA-376 Part 1, §15.2.4,
//! §15.2.5, §17.5.2.6): the XML mapping pane's commands. Every edit is one undo step of the
//! editing session (see `bo_edit::custom_xml`).

use crate::edit::EditResult;
use crate::{EngineError, Result};
use serde::Deserialize;

/// A custom XML command (`cmd` starts with `customXml`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum CustomXmlCommand {
    /// List the parts with their nodes.
    CustomXmlParts,
    /// Add a part holding `xml`.
    CustomXmlAdd {
        /// The XML text.
        xml: String,
    },
    /// Replace the XML of a part.
    CustomXmlReplace {
        /// Part index.
        index: usize,
        /// The XML text.
        xml: String,
    },
    /// Remove a part.
    CustomXmlRemove {
        /// Part index.
        index: usize,
    },
    /// Set the value of a node; bound controls follow.
    CustomXmlSetValue {
        /// Part index.
        index: usize,
        /// Node path (XPath subset).
        xpath: String,
        /// `xmlns:ns0='…'` declarations for the path.
        #[serde(default)]
        prefix_mappings: String,
        /// New value.
        value: String,
    },
    /// Bind the content control at the caret to a node.
    CustomXmlBind {
        /// Part index.
        index: usize,
        /// Node path.
        xpath: String,
        /// Prefix declarations for the path.
        #[serde(default)]
        prefix_mappings: String,
    },
    /// Remove the binding of the content control at the caret.
    CustomXmlUnbind,
}

impl CustomXmlCommand {
    /// Parses a JSON command when it is a custom XML command.
    pub fn parse(v: &serde_json::Value) -> Option<CustomXmlCommand> {
        let c = v.get("cmd")?.as_str()?;
        if !c.starts_with("customXml") {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }

    /// Changes the parts' structure or a control (refused while only filling in forms is
    /// allowed, §17.15.1.29); setting a node value is filling in.
    fn structural(&self) -> bool {
        !matches!(
            self,
            CustomXmlCommand::CustomXmlParts | CustomXmlCommand::CustomXmlSetValue { .. }
        )
    }
}

impl crate::Engine {
    /// Run a custom XML command on the document. Failures are reported in `error` (message
    /// keys `badXml`, `badPart`, `badXPath`, `noNode`, `notLeaf`, `noControl`, `locked`,
    /// `notBindable`, `tooLarge`, `protected`, `readOnly`).
    pub fn doc_custom_xml_command(&mut self, cmd: CustomXmlCommand) -> Result<EditResult> {
        if self.pres.is_some() {
            return Err(EngineError::BadCommand("not a document".into()));
        }
        let read_only = self.sec.read_only;
        let s = self.session.as_mut().ok_or(EngineError::NoDocument)?;
        let res: std::result::Result<(), String> = if read_only
            && !matches!(cmd, CustomXmlCommand::CustomXmlParts)
        {
            Err(crate::security::READ_ONLY.into())
        } else if cmd.structural() && s.forms_locked() {
            Err("protected".into())
        } else {
            let key = |e: bo_edit::EditError| match e {
                bo_edit::EditError::Unsupported(k) => k,
                other => other.to_string(),
            };
            match cmd {
                CustomXmlCommand::CustomXmlParts => Ok(()),
                CustomXmlCommand::CustomXmlAdd { xml } => {
                    s.add_custom_xml_part(&xml).map(|_| ()).map_err(key)
                }
                CustomXmlCommand::CustomXmlReplace { index, xml } => s
                    .replace_custom_xml_part(index, &xml)
                    .map(|_| ())
                    .map_err(key),
                CustomXmlCommand::CustomXmlRemove { index } => {
                    s.remove_custom_xml_part(index).map_err(key)
                }
                CustomXmlCommand::CustomXmlSetValue {
                    index,
                    xpath,
                    prefix_mappings,
                    value,
                } => s
                    .set_custom_xml_value(index, &xpath, &prefix_mappings, &value)
                    .map(|_| ())
                    .map_err(key),
                CustomXmlCommand::CustomXmlBind {
                    index,
                    xpath,
                    prefix_mappings,
                } => s.bind_control(index, &xpath, &prefix_mappings).map_err(key),
                CustomXmlCommand::CustomXmlUnbind => s.unbind_control().map(|_| ()).map_err(key),
            }
        };
        let parts = s.custom_xml_parts();
        let dirty = self.relayout()?;
        let mut r = self.story_result(dirty, res.err())?;
        r.custom_xml = Some(parts);
        r.collab = self.collab_after_edit();
        Ok(r)
    }
}
