//! @mentions and tasks of comments (CORE-COMMENT-001, see `bo_model::mention`).
//!
//! - A mention is a run "@Name" inside a `mailto:` hyperlink in the comment
//!   (ECMA-376 Part 1, §17.16.22 hyperlink); the person is listed in the people
//!   part ([MS-DOCX] §2.3.x people part, `w15:people` / `w15:person` with
//!   `w15:presenceInfo`), which new mentions extend.
//! - Tasks have no WordprocessingML schema: BlueOffice keeps them in its own
//!   comment tasks part (`bo:commentTasks`, one `bo:task` per comment keyed by
//!   the comment's `w14:paraId`), related from the main document with
//!   [`REL_TASKS`]. Other software ignores the unknown relationship.

use bo_model::mention::{mailto, CommentTask, MentionPerson, NS as BO_NS};
use bo_model::{Block, Comment};
use bo_opc::xml::escape;
use std::fmt::Write as _;

/// [MS-DOCX] people part relationship (main document → people part).
pub(crate) const REL_PEOPLE: &str = "http://schemas.microsoft.com/office/2011/relationships/people";
/// [MS-DOCX] people part content type.
pub(crate) const CT_PEOPLE: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.people+xml";
/// BlueOffice comment tasks part relationship.
pub(crate) const REL_TASKS: &str =
    "http://schemas.bluestar.vn/blueoffice/2026/relationships/commentTasks";
/// BlueOffice comment tasks part content type.
pub(crate) const CT_TASKS: &str = "application/vnd.bluestar.blueoffice.commentTasks+xml";
const W15_NS: &str = "http://schemas.microsoft.com/office/word/2012/wordml";
const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// Persons mentioned in the comments: `mailto:` hyperlink runs whose text starts with `@`.
pub(crate) fn mentioned(comments: &[Comment]) -> Vec<MentionPerson> {
    let mut out: Vec<MentionPerson> = Vec::new();
    for c in comments {
        for b in &c.blocks {
            let Block::Paragraph(p) = b else { continue };
            for r in &p.runs {
                let Some(mail) = r.props.link.as_deref().and_then(mailto) else {
                    continue;
                };
                let text = r.plain_text();
                let Some(name) = text.strip_prefix('@') else {
                    continue;
                };
                if !out
                    .iter()
                    .any(|m| m.email.as_deref() == Some(mail.as_str()))
                {
                    out.push(MentionPerson {
                        id: String::new(),
                        name: name.trim().to_owned(),
                        email: Some(mail),
                    });
                }
            }
        }
    }
    out
}

/// `w15:author` values of a people part.
fn people_authors(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = xml[from..].find(":author=\"") {
        let at = from + i + ":author=\"".len();
        let Some(end) = xml[at..].find('"') else {
            break;
        };
        out.push(bo_opc::xml::unescape(&xml[at..at + end]));
        from = at + end;
    }
    out
}

fn person_xml(p: &MentionPerson) -> String {
    // [MS-DOCX] CT_Person: author (required), presenceInfo (providerId, userId).
    let mut s = format!("<w15:person w15:author=\"{}\">", escape(&p.name));
    if let Some(e) = &p.email {
        let _ = write!(
            s,
            "<w15:presenceInfo w15:providerId=\"None\" w15:userId=\"{}\"/>",
            escape(e)
        );
    }
    s.push_str("</w15:person>");
    s
}

/// The people part with every person of `add` that it does not list yet;
/// `None` when nothing is missing (the part stays as it is).
pub(crate) fn people_xml(existing: Option<&str>, add: &[MentionPerson]) -> Option<String> {
    let known = existing.map(people_authors).unwrap_or_default();
    let missing: Vec<&MentionPerson> = add
        .iter()
        .filter(|p| !p.name.is_empty() && !known.contains(&p.name))
        .collect();
    if missing.is_empty() {
        return None;
    }
    let new: String = missing.iter().map(|p| person_xml(p)).collect();
    match existing {
        Some(xml) if xml.contains("</w15:people>") => {
            Some(xml.replacen("</w15:people>", &format!("{new}</w15:people>"), 1))
        }
        Some(xml) if xml.trim_end().ends_with("/>") && xml.contains("w15:people") => {
            let t = xml.trim_end().trim_end_matches("/>");
            Some(format!("{t}>{new}</w15:people>"))
        }
        _ => Some(format!(
            "{XML_DECL}<w15:people xmlns:w15=\"{W15_NS}\" xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\" mc:Ignorable=\"w15\">{new}</w15:people>"
        )),
    }
}

/// The BlueOffice comment tasks part of `comments` (`None`: no task).
pub(crate) fn tasks_xml(comments: &[Comment]) -> Option<String> {
    let tasks: Vec<(&str, &CommentTask)> = comments
        .iter()
        .filter_map(|c| Some((c.para_id.as_deref()?, c.task.as_ref()?)))
        .collect();
    if tasks.is_empty() {
        return None;
    }
    let mut s = format!("{XML_DECL}<bo:commentTasks xmlns:bo=\"{BO_NS}\">");
    for (pid, t) in tasks {
        let _ = write!(
            s,
            "<bo:task paraId=\"{}\" assigneeId=\"{}\" assigneeName=\"{}\"",
            escape(pid),
            escape(&t.assignee.id),
            escape(&t.assignee.name)
        );
        if let Some(e) = &t.assignee.email {
            let _ = write!(s, " assigneeEmail=\"{}\"", escape(e));
        }
        s.push_str("/>");
    }
    s.push_str("</bo:commentTasks>");
    Some(s)
}

/// Attach the tasks of a comment tasks part to their comments (by `paraId`).
pub(crate) fn read_tasks(data: &[u8], comments: &mut [Comment]) {
    let xml = String::from_utf8_lossy(data);
    let attr = |tag: &str, name: &str| -> Option<String> {
        let pat = format!(" {name}=\"");
        let i = tag.find(&pat)? + pat.len();
        let end = tag[i..].find('"')?;
        Some(bo_opc::xml::unescape(&tag[i..i + end]))
    };
    let mut from = 0;
    while let Some(i) = xml[from..].find("<bo:task ") {
        let at = from + i;
        let Some(end) = xml[at..].find('>') else {
            break;
        };
        let tag = &xml[at..at + end];
        from = at + end;
        let Some(pid) = attr(tag, "paraId") else {
            continue;
        };
        if let Some(c) = comments
            .iter_mut()
            .find(|c| c.para_id.as_deref() == Some(pid.as_str()))
        {
            c.task = Some(CommentTask {
                assignee: MentionPerson {
                    id: attr(tag, "assigneeId").unwrap_or_default(),
                    name: attr(tag, "assigneeName").unwrap_or_default(),
                    email: attr(tag, "assigneeEmail"),
                },
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn people_part_is_extended_not_rewritten() {
        let src = format!("{XML_DECL}<w15:people xmlns:w15=\"{W15_NS}\"><w15:person w15:author=\"An\"><w15:presenceInfo w15:providerId=\"AD\" w15:userId=\"S-1\"/></w15:person></w15:people>");
        let an = MentionPerson {
            id: String::new(),
            name: "An".into(),
            email: Some("an@x.invalid".into()),
        };
        assert_eq!(people_xml(Some(&src), std::slice::from_ref(&an)), None);
        let binh = MentionPerson {
            name: "Bình & Co".into(),
            email: Some("binh@x.invalid".into()),
            ..MentionPerson::default()
        };
        let out = people_xml(Some(&src), &[an, binh]).unwrap();
        assert!(out.contains("w15:userId=\"S-1\""));
        assert!(out.contains("w15:author=\"Bình &amp; Co\""));
        assert_eq!(people_authors(&out), vec!["An", "Bình & Co"]);
    }
}
