//! Modern (threaded) slide comments ([MS-PPTX] PowerPoint 2018 comments:
//! comment part `p188:cmLst` / `p188:cm` with `p188:replyLst`, and the authors
//! part `p188:authorLst` / `p188:author`). Legacy comments (ECMA-376 Part 1,
//! §19.4 `p:cmLst`) stay verbatim in the package.
//!
//! [MS-PPTX] defines no mention element for modern comments: a mention stays
//! "@Name" in the text and the BlueOffice extension (`bo_model::mention`, one
//! `a:ext` of the comment's `p188:extLst`) names the person and the assigned
//! task. The text body and the other extensions of a comment read from a file
//! are written back unchanged.

use bo_model::mention::{CommentTask, Mention, MentionPerson, EXT_URI, NS as BO_NS};
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
    /// @mentions in `text` (BlueOffice extension; UTF-16 ranges).
    pub mentions: Vec<Mention>,
    /// Task assigned in the comment (BlueOffice extension).
    pub task: Option<CommentTask>,
    /// `p188:txBody` as read (written back while `text` is unchanged).
    pub raw_body: Option<String>,
    /// Other `ext` elements of `p188:extLst`, kept verbatim.
    pub raw_ext: Vec<String>,
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
    /// `p188:pos` ([MS-PPTX] 2.3.x CT_Comment, a:CT_Point2D): where the comment marker sits
    /// on the slide, EMU.
    pub pos: Option<(i64, i64)>,
    /// Shape the thread is anchored to ([MS-PPTX] `ac:deMkLst` / `ac:spMk@id`).
    pub shape: Option<u32>,
    /// The `ac:deMkLst` moniker list as read (written back verbatim).
    pub anchor_raw: Option<String>,
}

/// `ac` namespace of drawing element monikers ([MS-PPTX] 2013 drawing command).
pub const NS_AC: &str = "http://schemas.microsoft.com/office/drawing/2013/main/command";

/// Relationship slide → legacy comments part (ECMA-376 Part 1, §13.3.? comments).
pub const REL_LEGACY_COMMENTS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/comments";
/// Relationship presentation → legacy comment authors part (§19.4.3 cmAuthorLst).
pub const REL_LEGACY_AUTHORS: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/commentAuthors";

/// A legacy comment (ECMA-376 Part 1, §19.4.1 cm), read only.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LegacyComment {
    /// `authorId` → [`LegacyAuthor::id`].
    pub author_id: u32,
    /// `dt` (xsd:dateTime).
    pub dt: String,
    /// `idx`: number of the comment for its author.
    pub idx: u32,
    /// §19.4.7 pos: x, y in 1/8 point (master units of 576 per inch).
    pub pos: Option<(i64, i64)>,
    /// §19.4.8 text.
    pub text: String,
}

/// A legacy comment author (§19.4.2 cmAuthor).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LegacyAuthor {
    /// `id`.
    pub id: u32,
    /// `name`.
    pub name: String,
    /// `initials`.
    pub initials: String,
}

/// Comments of a legacy comments part (§19.4.4 cmLst), in file order.
pub fn read_legacy_comments(data: &[u8], part: &str) -> Vec<LegacyComment> {
    let Ok(root) = bo_drawingml::dom::parse(data, part) else {
        return Vec::new();
    };
    if root.local() != "cmLst" {
        return Vec::new();
    }
    let num = |e: &XmlElement, k: &str| e.attr(k).and_then(|v| v.trim().parse::<u32>().ok());
    let int = |e: &XmlElement, k: &str| e.attr(k).and_then(|v| v.trim().parse::<i64>().ok());
    kids(&root, "cm")
        .map(|cm| LegacyComment {
            author_id: num(cm, "authorId").unwrap_or(0),
            dt: cm.attr("dt").unwrap_or_default().to_owned(),
            idx: num(cm, "idx").unwrap_or(0),
            pos: kids(cm, "pos")
                .next()
                .and_then(|p| Some((int(p, "x")?, int(p, "y")?))),
            text: kids(cm, "text")
                .next()
                .map(XmlElement::text)
                .unwrap_or_default(),
        })
        .collect()
}

/// Authors of a legacy comment authors part (§19.4.3 cmAuthorLst).
pub fn read_legacy_authors(data: &[u8], part: &str) -> Vec<LegacyAuthor> {
    let Ok(root) = bo_drawingml::dom::parse(data, part) else {
        return Vec::new();
    };
    kids(&root, "cmAuthor")
        .map(|a| LegacyAuthor {
            id: a
                .attr("id")
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(0),
            name: a.attr("name").unwrap_or_default().to_owned(),
            initials: a.attr("initials").unwrap_or_default().to_owned(),
        })
        .collect()
}

/// The shape id named by a moniker list (`ac:spMk@id`, at any depth).
fn moniker_shape(el: &XmlElement) -> Option<u32> {
    if el.local() == "spMk" {
        return el.attr("id").and_then(|v| v.parse().ok());
    }
    el.elements().find_map(moniker_shape)
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

fn xml_of(el: &XmlElement) -> String {
    let mut s = String::new();
    bo_drawingml::dom::write_element(el, &mut s);
    s
}

fn person_of(el: &XmlElement, prefix: &str) -> MentionPerson {
    let a = |k: &str| el.attr(&format!("{prefix}{k}")).map(str::to_owned);
    MentionPerson {
        id: a("Id").unwrap_or_default(),
        name: a("Name").unwrap_or_default(),
        email: a("Email"),
    }
}

fn reply_of(el: &XmlElement) -> ModernReply {
    let body = kids(el, "txBody").next();
    let mut r = ModernReply {
        id: el.attr("id").unwrap_or_default().to_owned(),
        author_id: el.attr("authorId").unwrap_or_default().to_owned(),
        created: el.attr("created").unwrap_or_default().to_owned(),
        text: body.map(body_text).unwrap_or_default(),
        raw_body: body.map(xml_of),
        ..ModernReply::default()
    };
    for ext in kids(el, "extLst").flat_map(|l| kids(l, "ext")) {
        if ext.attr("uri") != Some(EXT_URI) {
            r.raw_ext.push(xml_of(ext));
            continue;
        }
        for c in ext.elements() {
            match c.local() {
                "mention" => r.mentions.push(Mention {
                    person: person_of(c, "person"),
                    start: c.attr("start").and_then(|v| v.parse().ok()).unwrap_or(0),
                    length: c.attr("length").and_then(|v| v.parse().ok()).unwrap_or(0),
                }),
                "task" => {
                    r.task = Some(CommentTask {
                        assignee: person_of(c, "assignee"),
                    });
                }
                _ => {}
            }
        }
    }
    r
}

/// `xmlns:*` declarations of a part's root element (kept when the part is
/// rewritten, so verbatim bodies and extensions keep their prefixes).
pub fn root_namespaces(data: &[u8], part: &str) -> Vec<(String, String)> {
    bo_drawingml::dom::parse(data, part)
        .map(|root| {
            root.attrs
                .into_iter()
                .filter(|(k, _)| k.starts_with("xmlns:"))
                .collect()
        })
        .unwrap_or_default()
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
            pos: kids(cm, "pos")
                .next()
                .and_then(|p| Some((p.attr("x")?.parse().ok()?, p.attr("y")?.parse().ok()?))),
            shape: kids(cm, "deMkLst").next().and_then(moniker_shape),
            anchor_raw: kids(cm, "deMkLst").next().map(xml_of),
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

fn person_attrs(out: &mut String, prefix: &str, p: &MentionPerson) {
    let _ = write!(
        out,
        " {prefix}Id=\"{}\" {prefix}Name=\"{}\"",
        escape(&p.id),
        escape(&p.name)
    );
    if let Some(e) = &p.email {
        let _ = write!(out, " {prefix}Email=\"{}\"", escape(e));
    }
}

/// `p188:txBody` (verbatim when read and unchanged) and `p188:extLst`.
fn write_content(out: &mut String, r: &ModernReply) {
    match &r.raw_body {
        Some(raw)
            if bo_drawingml::dom::parse(raw.as_bytes(), "txBody")
                .is_ok_and(|b| body_text(&b) == r.text) =>
        {
            out.push_str(raw);
        }
        _ => write_body(out, &r.text),
    }
    if r.raw_ext.is_empty() && r.mentions.is_empty() && r.task.is_none() {
        return;
    }
    out.push_str("<p188:extLst>");
    for e in &r.raw_ext {
        out.push_str(e);
    }
    if !r.mentions.is_empty() || r.task.is_some() {
        let _ = write!(out, "<a:ext uri=\"{EXT_URI}\">");
        for m in &r.mentions {
            let _ = write!(out, "<bo:mention xmlns:bo=\"{BO_NS}\"");
            person_attrs(out, "person", &m.person);
            let _ = write!(out, " start=\"{}\" length=\"{}\"/>", m.start, m.length);
        }
        if let Some(t) = &r.task {
            let _ = write!(out, "<bo:task xmlns:bo=\"{BO_NS}\"");
            person_attrs(out, "assignee", &t.assignee);
            out.push_str("/>");
        }
        out.push_str("</a:ext>");
    }
    out.push_str("</p188:extLst>");
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
    comments_xml_ns(comments, sld_id, creation_id, &[])
}

/// [`comments_xml`] also declaring the namespaces `ns` of the part it replaces
/// (see [`root_namespaces`]).
pub fn comments_xml_ns(
    comments: &[ModernComment],
    sld_id: u32,
    creation_id: u32,
    ns: &[(String, String)],
) -> String {
    let mut out = String::from(XML_DECL);
    let _ = write!(
        out,
        "<p188:cmLst xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" xmlns:p188=\"{NS_P188}\""
    );
    for (k, v) in ns {
        if !matches!(k.as_str(), "xmlns:a" | "xmlns:r" | "xmlns:p188") {
            let _ = write!(out, " {k}=\"{}\"", escape(v));
        }
    }
    out.push('>');
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
        // A thread on a shape names it in a drawing element moniker list, else the slide.
        match (&c.anchor_raw, c.shape) {
            (Some(raw), _) => {
                out.push('>');
                out.push_str(raw);
            }
            (None, Some(id)) => {
                let _ = write!(
                    out,
                    "><ac:deMkLst xmlns:ac=\"{NS_AC}\"><pc:docMk xmlns:pc=\"{NS_PC}\"/><pc:sldMk xmlns:pc=\"{NS_PC}\" cId=\"{creation_id}\" sldId=\"{sld_id}\"/><ac:spMk id=\"{id}\"/></ac:deMkLst>"
                );
            }
            (None, None) => {
                let _ = write!(
                    out,
                    "><pc:sldMkLst xmlns:pc=\"{NS_PC}\"><pc:docMk/><pc:sldMk cId=\"{creation_id}\" sldId=\"{sld_id}\"/></pc:sldMkLst>"
                );
            }
        }
        if let Some((x, y)) = c.pos {
            let _ = write!(out, "<p188:pos x=\"{x}\" y=\"{y}\"/>");
        }
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
                write_content(&mut out, r);
                out.push_str("</p188:reply>");
            }
            out.push_str("</p188:replyLst>");
        }
        write_content(&mut out, f);
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
    fn shape_anchor_is_written_and_read() {
        let c = vec![ModernComment {
            first: ModernReply {
                id: "{C1}".into(),
                author_id: "{A1}".into(),
                created: "2026-11-16T10:00:00.000".into(),
                text: "On the logo".into(),
                ..ModernReply::default()
            },
            shape: Some(7),
            ..ModernComment::default()
        }];
        let xml = comments_xml(&c, 256, 0);
        assert!(xml.contains("<ac:spMk id=\"7\"/>"), "{xml}");
        let back = read_comments(xml.as_bytes(), "/ppt/comments/c.xml");
        assert_eq!(back[0].shape, Some(7));
        // Read back, the moniker list is written verbatim.
        let again = comments_xml(&back, 256, 0);
        assert_eq!(
            read_comments(again.as_bytes(), "/c.xml")[0].anchor_raw,
            back[0].anchor_raw
        );
    }

    #[test]
    fn legacy_comments_and_authors_read() {
        let cm = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:cmLst xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cm authorId="1" dt="2010-05-01T10:00:00.000" idx="1"><p:pos x="10" y="20"/><p:text>Kiểm tra số liệu</p:text></p:cm></p:cmLst>"#;
        let au = r#"<p:cmAuthorLst xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cmAuthor id="1" name="Trần Bình" initials="TB" lastIdx="1" clrIdx="0"/></p:cmAuthorLst>"#;
        let c = read_legacy_comments(cm.as_bytes(), "/ppt/comments/comment1.xml");
        assert_eq!(c.len(), 1);
        assert_eq!((c[0].author_id, c[0].idx, c[0].pos), (1, 1, Some((10, 20))));
        assert_eq!(c[0].text, "Kiểm tra số liệu");
        let a = read_legacy_authors(au.as_bytes(), "/ppt/commentAuthors.xml");
        assert_eq!(a[0].name, "Trần Bình");
    }

    #[test]
    fn comment_and_author_parts_round_trip() {
        let r = |id: &str, text: &str| ModernReply {
            id: id.into(),
            author_id: "{A1}".into(),
            created: "2026-11-08T10:00:00.000".into(),
            text: text.into(),
            ..ModernReply::default()
        };
        let c = vec![ModernComment {
            first: r("{C1}", "Sửa tiêu đề <slide 1>\nvà ảnh & màu"),
            resolved: true,
            replies: vec![r("{R1}", "Đã sửa")],
            pos: Some((1_524_000, 762_000)),
            shape: None,
            anchor_raw: None,
        }];
        let xml = comments_xml(&c, 256, 0);
        let mut back = read_comments(xml.as_bytes(), "/ppt/comments/modernComment_100_0.xml");
        for x in &mut back {
            x.first.raw_body = None;
            for y in &mut x.replies {
                y.raw_body = None;
            }
        }
        assert_eq!(back, c);
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

    #[test]
    fn mentions_tasks_and_foreign_extensions_survive_a_rewrite() {
        let src = format!(
            "{XML_DECL}<p188:cmLst xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" xmlns:p188=\"{NS_P188}\" xmlns:x9=\"urn:x9\"><p188:cm id=\"{{C1}}\" authorId=\"{{A1}}\" created=\"2026-01-01T00:00:00Z\"><p188:txBody><a:bodyPr/><a:p><a:r><a:rPr b=\"1\"/><a:t>Xem @Bình</a:t></a:r></a:p></p188:txBody><p188:extLst><a:ext uri=\"{{X}}\"><x9:thing v=\"1\"/></a:ext></p188:extLst></p188:cm></p188:cmLst>"
        );
        let part = "/ppt/comments/modernComment_100_0.xml";
        let mut list = read_comments(src.as_bytes(), part);
        assert_eq!(list[0].first.text, "Xem @Bình");
        let binh = MentionPerson {
            id: "u2".into(),
            name: "Bình".into(),
            email: Some("binh@x.invalid".into()),
        };
        list[0].first.mentions.push(Mention {
            person: binh.clone(),
            start: 4,
            length: 5,
        });
        list[0].first.task = Some(CommentTask { assignee: binh });
        let ns = root_namespaces(src.as_bytes(), part);
        let xml = comments_xml_ns(&list, 256, 0, &ns);
        // The bold run and the foreign extension are kept, with their namespace.
        assert!(xml.contains("<a:rPr b=\"1\"/>"), "{xml}");
        assert!(xml.contains("xmlns:x9=\"urn:x9\""));
        assert!(xml.contains("<x9:thing v=\"1\"/>"));
        let back = read_comments(xml.as_bytes(), part);
        assert_eq!(back[0].first.mentions, list[0].first.mentions);
        assert_eq!(back[0].first.task, list[0].first.task);
        assert_eq!(back[0].first.raw_ext, list[0].first.raw_ext);
    }
}
