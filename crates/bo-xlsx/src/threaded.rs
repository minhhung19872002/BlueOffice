//! Threaded comments ([MS-XLSX] threaded comments part, `CT_ThreadedComments` /
//! `CT_ThreadedComment`) and the person list ([MS-XLSX] person part,
//! `CT_PersonList` / `CT_Person`).
//!
//! A threaded comment is anchored on a cell (`ref`), written by a person
//! (`personId`), and replies point at the thread's first comment (`parentId`).
//! Next to them the sheet keeps a legacy note (§18.7 comments) per thread so
//! older readers still see the text.

use crate::chart::parse_dom;
use bo_model::sheet::{CellRef, Person, ThreadedComment};
use bo_opc::xml::escape;
use std::fmt::Write as _;

const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// Namespace of both parts ([MS-XLSX] threadedcomments 2018).
pub const NS: &str = "http://schemas.microsoft.com/office/spreadsheetml/2018/threadedcomments";
/// Relationship sheet → threaded comments part.
pub const REL_THREADED: &str =
    "http://schemas.microsoft.com/office/2017/10/relationships/threadedComment";
/// Relationship workbook → person part.
pub const REL_PERSON: &str = "http://schemas.microsoft.com/office/2017/10/relationships/person";
pub const CT_THREADED: &str = "application/vnd.ms-excel.threadedcomments+xml";
pub const CT_PERSON: &str = "application/vnd.ms-excel.person+xml";

/// `ThreadedComments` part: every `threadedComment` in file order.
pub(crate) fn read_threaded(data: &[u8], part: &str) -> Vec<ThreadedComment> {
    let Some(root) = parse_dom(data, part) else {
        return Vec::new();
    };
    root.children
        .iter()
        .filter(|n| n.name == "threadedComment")
        .filter_map(|n| {
            // CT_ThreadedComment: ref, dT, personId, id (required), parentId, done.
            let cell = CellRef::parse(n.attr("ref")?)?;
            Some(ThreadedComment {
                id: n.attr("id")?.to_owned(),
                cell,
                person_id: n.attr("personId").unwrap_or_default().to_owned(),
                date: n.attr("dT").map(str::to_owned),
                parent_id: n.attr("parentId").map(str::to_owned),
                text: n.child("text").map(|t| t.text.clone()).unwrap_or_default(),
                done: matches!(n.attr("done"), Some("1" | "true")),
            })
        })
        .collect()
}

/// `personList` part.
pub(crate) fn read_persons(data: &[u8], part: &str) -> Vec<Person> {
    let Some(root) = parse_dom(data, part) else {
        return Vec::new();
    };
    root.children
        .iter()
        .filter(|n| n.name == "person")
        .filter_map(|n| {
            // CT_Person: displayName, id (required), userId, providerId.
            Some(Person {
                id: n.attr("id")?.to_owned(),
                display_name: n.attr("displayName").unwrap_or_default().to_owned(),
                user_id: n.attr("userId").map(str::to_owned),
                provider_id: n.attr("providerId").map(str::to_owned),
            })
        })
        .collect()
}

pub(crate) fn threaded_xml(comments: &[ThreadedComment]) -> String {
    let mut out = String::from(XML_DECL);
    let _ = write!(out, "<ThreadedComments xmlns=\"{NS}\">");
    for c in comments {
        let _ = write!(out, "<threadedComment ref=\"{}\"", c.cell);
        if let Some(d) = &c.date {
            let _ = write!(out, " dT=\"{}\"", escape(d));
        }
        let _ = write!(
            out,
            " personId=\"{}\" id=\"{}\"",
            escape(&c.person_id),
            escape(&c.id)
        );
        if let Some(p) = &c.parent_id {
            let _ = write!(out, " parentId=\"{}\"", escape(p));
        }
        if c.done {
            out.push_str(" done=\"1\"");
        }
        let _ = write!(out, "><text>{}</text></threadedComment>", escape(&c.text));
    }
    out.push_str("</ThreadedComments>");
    out
}

pub(crate) fn persons_xml(persons: &[Person]) -> String {
    let mut out = String::from(XML_DECL);
    let _ = write!(
        out,
        "<personList xmlns=\"{NS}\" xmlns:x=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">"
    );
    for p in persons {
        let _ = write!(
            out,
            "<person displayName=\"{}\" id=\"{}\"",
            escape(&p.display_name),
            escape(&p.id)
        );
        if let Some(u) = &p.user_id {
            let _ = write!(out, " userId=\"{}\"", escape(u));
        }
        if let Some(pr) = &p.provider_id {
            let _ = write!(out, " providerId=\"{}\"", escape(pr));
        }
        out.push_str("/>");
    }
    out.push_str("</personList>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_parts() {
        let c = vec![
            ThreadedComment {
                id: "{00000000-0000-4000-8000-000000000001}".into(),
                cell: CellRef::new(1, 2),
                person_id: "{00000000-0000-4000-8000-0000000000aa}".into(),
                date: Some("2026-11-08T09:30:00.00".into()),
                parent_id: None,
                text: "Kiểm tra số liệu <quý 3> & ghi chú".into(),
                done: true,
            },
            ThreadedComment {
                id: "{00000000-0000-4000-8000-000000000002}".into(),
                cell: CellRef::new(1, 2),
                person_id: "{00000000-0000-4000-8000-0000000000aa}".into(),
                date: None,
                parent_id: Some("{00000000-0000-4000-8000-000000000001}".into()),
                text: "Đã sửa".into(),
                done: false,
            },
        ];
        let xml = threaded_xml(&c);
        assert_eq!(read_threaded(xml.as_bytes(), "/xl/threadedComments/t1.xml"), c);
        let p = vec![Person {
            id: "{00000000-0000-4000-8000-0000000000aa}".into(),
            display_name: "Nguyễn Văn An".into(),
            user_id: Some("an@example.invalid".into()),
            provider_id: Some("None".into()),
        }];
        assert_eq!(read_persons(persons_xml(&p).as_bytes(), "/xl/persons/person.xml"), p);
    }
}
