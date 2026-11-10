//! @mentions and assigned tasks of comments (CORE-COMMENT-001), shared by the
//! three apps.
//!
//! Where each format keeps them:
//! - `.xlsx`: [MS-XLSX] threaded comments `mentions/mention` (`mentionpersonId`,
//!   `mentionId`, `startIndex`, `length`) + the person part.
//! - `.docx`: the mention is a `mailto:` hyperlink run "@Name" in the comment
//!   (ECMA-376 Part 1, §17.16.22) and the person is listed in the [MS-DOCX]
//!   people part (`w15:people` / `w15:person`).
//! - `.pptx`: [MS-PPTX] modern comments define no mention element: the text keeps
//!   "@Name" and the BlueOffice extension below names the person.
//!
//! Tasks have no standard schema in any of them; BlueOffice stores the assignee in
//! its own extension that other software ignores (docs/comment-mentions.md):
//! an `ext` of the comment's `extLst` (xlsx, pptx) or the comment tasks part
//! (docx), all in [`NS`].

use serde::{Deserialize, Serialize};

/// Namespace of the BlueOffice comment extension.
pub const NS: &str = "http://schemas.bluestar.vn/blueoffice/2026/comments";
/// `uri` of the BlueOffice `ext` inside a comment's `extLst` (ECMA-376 Part 1, §18.2.7 / DrawingML `CT_OfficeArtExtension`).
pub const EXT_URI: &str = "{B10E0FF1-CE00-4A55-9C0D-7A5C0000C0DE}";

/// A person who can be mentioned or assigned (a host user or a collaborator).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MentionPerson {
    /// Id at the host (BlueDrive user id); may be empty for people read from a file.
    #[serde(default)]
    pub id: String,
    /// Display name.
    pub name: String,
    /// E-mail address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

/// A mention inside a comment's text: `text[start..start + length]` (UTF-16 code
/// units, as [MS-XLSX] `startIndex`/`length` and JavaScript strings count) is "@Name".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mention {
    /// The person mentioned.
    #[serde(flatten)]
    pub person: MentionPerson,
    /// First UTF-16 code unit of the mention in the comment text.
    pub start: u32,
    /// Length in UTF-16 code units.
    pub length: u32,
}

/// A task assigned in a comment; it is complete when the thread is resolved.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentTask {
    /// Who the task is assigned to.
    pub assignee: MentionPerson,
}

/// Byte range of the UTF-16 range `[start, start + length)` of `text`; `None`
/// when it falls outside the text or inside a character.
#[must_use]
pub fn utf16_range(text: &str, start: u32, length: u32) -> Option<std::ops::Range<usize>> {
    let (start, end) = (start as usize, start as usize + length as usize);
    let mut units = 0usize;
    let (mut a, mut b) = (None, None);
    for (i, ch) in text.char_indices() {
        if units == start {
            a = Some(i);
        }
        if units == end {
            b = Some(i);
        }
        units += ch.len_utf16();
    }
    if units == start {
        a = Some(text.len());
    }
    if units == end {
        b = Some(text.len());
    }
    match (a, b) {
        (Some(a), Some(b)) if a <= b => Some(a..b),
        _ => None,
    }
}

/// Address of a `mailto:` URL (RFC 6068), without its query.
#[must_use]
pub fn mailto(link: &str) -> Option<String> {
    let rest = link
        .strip_prefix("mailto:")
        .or_else(|| link.strip_prefix("MAILTO:"))?;
    let addr = rest.split('?').next().unwrap_or_default().trim();
    (!addr.is_empty()).then(|| addr.to_owned())
}

/// UTF-16 length of `s`.
#[must_use]
pub fn utf16_len(s: &str) -> u32 {
    u32::try_from(s.encode_utf16().count()).unwrap_or(u32::MAX)
}

/// The mentions that fit `text` (inside it, not overlapping), sorted by start.
#[must_use]
pub fn valid_mentions(text: &str, mentions: &[Mention]) -> Vec<Mention> {
    let mut list: Vec<Mention> = mentions
        .iter()
        .filter(|m| m.length > 0 && utf16_range(text, m.start, m.length).is_some())
        .cloned()
        .collect();
    list.sort_by_key(|m| m.start);
    let mut end = 0;
    list.retain(|m| {
        let ok = m.start >= end;
        if ok {
            end = m.start + m.length;
        }
        ok
    });
    list
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf16_ranges() {
        let t = "Gửi @Bình 😀 @An";
        // "Gửi " = 4 units, "@Bình" = 5.
        assert_eq!(&t[utf16_range(t, 4, 5).unwrap()], "@Bình");
        // The emoji is two units.
        assert_eq!(&t[utf16_range(t, 13, 3).unwrap()], "@An");
        assert!(utf16_range(t, 11, 1).is_none());
        assert!(utf16_range(t, 14, 9).is_none());
    }

    #[test]
    fn mailto_addresses() {
        assert_eq!(
            mailto("mailto:an@x.invalid?subject=a").as_deref(),
            Some("an@x.invalid")
        );
        assert_eq!(mailto("https://x"), None);
    }

    #[test]
    fn overlapping_mentions_are_dropped() {
        let m = |start, length| Mention {
            person: MentionPerson::default(),
            start,
            length,
        };
        let v = valid_mentions("@An @Bình", &[m(4, 5), m(0, 3), m(1, 2), m(8, 4)]);
        assert_eq!(v, vec![m(0, 3), m(4, 5)]);
    }
}
