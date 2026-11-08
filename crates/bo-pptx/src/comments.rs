//! Modern (threaded) slide comments ([MS-PPTX] PowerPoint 2018 comments:
//! comment part `p188:cmLst` / `p188:cm` with `p188:replyLst`, and the authors
//! part `p188:authorLst` / `p188:author`). Legacy comments (ECMA-376 Part 1,
//! §19.4 `p:cmLst`) stay verbatim in the package.

use bo_model::slide::{XmlElement, XmlNode};
use bo_opc::xml::escape;
use std::fmt::Write as _;

/// `p188` namespace ([MS-PPTX] PowerPoint 2018 main).
pub const NS_P188: &str = "http://schemas.microsoft.com/office/powerpoint/2018/8/main";
/// `pc` namespace of the comment's slide moniker ([MS-PPTX] 2013 command).
pub const NS_PC: &str = "http://schemas.microsoft.com/office/powerpoint/2013/main/command";
/// Relationship slide → comment part.
pub const REL_COMMENTS: &str = "http://schemas.microsoft.com/office/2018/10/relationships/comments";
/// Relationship presentation → authors part.
pub const REL_AUTHORS: &str = "http://schemas.microsoft.com/office/2018/10/relationships/authors";
/// Content type of a comment part.
pub const CT_COMMENTS: &str = "application/vnd.ms-powerpoint.comments+xml";
/// Content type of the authors part.
pub const CT_AUTHORS: &str = "application/vnd.ms-powerpoint.authors+xml";

const XML_DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";

/// One comment or reply.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModernReply {
    /// `id` (GUID).
    pub id: String,
    /// `authorId` → [`CommentAuthor::id`].
    pub author_id: String,
    /// `created` (xsd:dateTime).
    pub created: String,
    /// Plain text of `p188:txBody`.
    pub text: String,
}

/// A thread: its first comment, `status` and replies (`p188:cm`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ModernComment {
    /// The comment opening the thread.
    pub first: ModernReply,
    /// `status="resolved"`.
    pub resolved: bool,
    /// `p188:replyLst`.
    pub replies: Vec<ModernReply>,
}

/// `p188:author`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommentAuthor {
    /// `id` (GUID).
    pub id: String,
    /// `name`.
    pub name: String,
    /// `initials`.
    pub initials: String,
    /// `userId`.
    pub user_id: String,
    /// `providerId`.
    pub provider_id: String,
}

fn kids<'a>(el: &'a XmlElement, local: &'a str) -> impl Iterator<Item = &'a XmlElement> + 'a {
    el.elements().filter(move |c| c.local() == local)
}

/// Plain text of a `p188:txBody` (DrawingML paragraphs joined by `\n`).
fn body_text(el: &XmlElement) -> String {
    fn texts(el: &XmlElement, out: &mut String) {
        for c in &el.children {
            match c {
                XmlNode::Element(e) if e.local() == "t" => {
                    for t in &e.children {
                        if let XmlNode::Text(s) = t {
                            out.push_str(s);
                        }
                    }
                }
                XmlNode::Element(e) => texts(e, out),
                XmlNode::Text(_) => {}
            }
        }
    }
    kids(el, "p")
        .map(|p| {
            let mut s = String::new();
            texts(p, &mut s);
            s
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn reply_of(el: &XmlElement) -> ModernReply {
    ModernReply {
        id: el.attr("id").unwrap_or_default().to_owned(),
        author_id: el.attr("authorId").unwrap_or_default().to_owned(),
        created: el.attr("created").unwrap_or_default().to_owned(),
        text: kids(el, "txBody").next().map(body_text).unwrap_or_default(),
    }
}

/// Comments of a comment part, in file order.
pub fn read_comments(data: &[u8], part: &str) -> Vec<ModernComment> {
    let Ok(root) = bo_drawingml::dom::parse(data, part) else {
        return Vec::new();
    };
    if root.local() != "cmLst" {
        return Vec::new();
    }
    kids(&root, "cm")
        .map(|cm| ModernComment {
            first: reply_of(cm),
            resolved: matches!(cm.attr("status"), Some("resolved" | "closed")),
            replies: kids(cm, "replyLst")
                .flat_map(|l| kids(l, "reply"))
                .map(reply_of)
                .collect(),
        })
        .collect()
}

/// Authors of an authors part.
pub fn read_authors(data: &[u8], part: &str) -> Vec<CommentAuthor> {
    let Ok(root) = bo_drawingml::dom::parse(data, part) else {
        return Vec::new();
    };
    kids(&root, "author")
        .map(|a| {
            let s = |k: &str| a.attr(k).unwrap_or_default().to_owned();
            CommentAuthor {
                id: s("id"),
                name: s("name"),
                initials: s("initials"),
                user_id: s("userId"),
                provider_id: s("providerId"),
            }
        })
        .collect()
}

fn write_body(out: &mut String, text: &str) {
    out.push_str("<p188:txBody><a:bodyPr/><a:lstStyle/>");
    for line in text.split('\n') {
        let _ = write!(
            out,
            "<a:p><a:r><a:rPr lang=\"vi-VN\"/><a:t>{}</a:t></a:r></a:p>",
            escape(line)
        );
    }
    out.push_str("</p188:txBody>");
}

/// A comment part for the slide with `p:sldId@id` = `sld_id` and creation id
/// `creation_id` (`p14:creationId`, 0 when unknown).
pub fn comments_xml(comments: &[ModernComment], sld_id: u32, creation_id: u32) -> String {
    let mut out = String::from(XML_DECL);
    let _ = write!(
        out,
        "<p188:cmLst xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p188=\"{NS_P188}\">"
    );
    for c in comments {
        let f = &c.first;
        let _ = write!(
            out,
            "<p188:cm id=\"{}\" authorId=\"{}\" created=\"{}\"",
            escape(&f.id),
            escape(&f.author_id),
            escape(&f.created)
        );
        if c.resolved {
            out.push_str(" status=\"resolved\"");
        }
        let _ = write!(
            out,
            "><pc:sldMkLst xmlns:pc=\"{NS_PC}\"><pc:docMk/><pc:sldMk cId=\"{creation_id}\" sldId=\"{sld_id}\"/></pc:sldMkLst>"
        );
        if !c.replies.is_empty() {
            out.push_str("<p188:replyLst>");
            for r in &c.replies {
                let _ = write!(
                    out,
                    "<p188:reply id=\"{}\" authorId=\"{}\" created=\"{}\">",
                    escape(&r.id),
                    escape(&r.author_id),
                    escape(&r.created)
                );
                write_body(&mut out, &r.text);
                out.push_str("</p188:reply>");
            }
            out.push_str("</p188:replyLst>");
        }
        write_body(&mut out, &f.text);
        out.push_str("</p188:cm>");
    }
    out.push_str("</p188:cmLst>");
    out
}

/// An authors part.
pub fn authors_xml(authors: &[CommentAuthor]) -> String {
    let mut out = String::from(XML_DECL);
    let _ = write!(
        out,
        "<p188:authorLst xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p188=\"{NS_P188}\">"
    );
    for a in authors {
        let _ = write!(
            out,
            "<p188:author id=\"{}\" name=\"{}\" initials=\"{}\" userId=\"{}\" providerId=\"{}\"/>",
            escape(&a.id),
            escape(&a.name),
            escape(&a.initials),
            escape(&a.user_id),
            escape(&a.provider_id)
        );
    }
    out.push_str("</p188:authorLst>");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comment_and_author_parts_round_trip() {
        let r = |id: &str, text: &str| ModernReply {
            id: id.into(),
            author_id: "{A1}".into(),
            created: "2026-11-08T10:00:00.000".into(),
            text: text.into(),
        };
        let c = vec![ModernComment {
            first: r("{C1}", "Sửa tiêu đề <slide 1>\nvà ảnh & màu"),
            resolved: true,
            replies: vec![r("{R1}", "Đã sửa")],
        }];
        let xml = comments_xml(&c, 256, 0);
        assert_eq!(
            read_comments(xml.as_bytes(), "/ppt/comments/modernComment_100_0.xml"),
            c
        );
        let a = vec![CommentAuthor {
            id: "{A1}".into(),
            name: "Nguyễn Văn An".into(),
            initials: "NA".into(),
            user_id: "an".into(),
            provider_id: "None".into(),
        }];
        assert_eq!(
            read_authors(authors_xml(&a).as_bytes(), "/ppt/authors.xml"),
            a
        );
    }
}
