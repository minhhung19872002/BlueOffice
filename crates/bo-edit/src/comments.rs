//! Comments with replies (ECMA-376 Part 1, §17.13.4; threading per [MS-DOCX]
//! §2.5.2 `w15:commentEx`): listing threads with their anchors in the main text
//! and the content of new comments.

use crate::para;
use crate::Pos;
use bo_model::index::{body_paragraphs, para_at};
use bo_model::{Block, Comment, Document, Paragraph, Raw, Run, RunContent, RunProps};
use serde::Serialize;

/// A comment for the comments pane (threads are rebuilt from `parent`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentInfo {
    pub id: i32,
    pub author: String,
    pub initials: Option<String>,
    pub date: Option<String>,
    pub text: String,
    pub done: bool,
    /// Id of the comment this one replies to.
    pub parent: Option<i32>,
    /// Anchor range in the main text (missing for comments without range marks).
    pub start: Option<Pos>,
    pub end: Option<Pos>,
    /// Commented text (shortened).
    pub quote: String,
}

/// Positions of every comment mark in the main text: id → (start, end, reference).
pub(crate) fn anchors(doc: &Document) -> std::collections::HashMap<i32, [Option<Pos>; 3]> {
    let mut m: std::collections::HashMap<i32, [Option<Pos>; 3]> = std::collections::HashMap::new();
    for (pid, path) in body_paragraphs(doc).iter().enumerate() {
        let Some(p) = para_at(doc, path) else {
            continue;
        };
        let mut off = 0;
        for (_, item) in para::flatten(p) {
            let at = Some(Pos::new(pid, off));
            match item {
                RunContent::CommentStart(id) => m.entry(id).or_default()[0] = at,
                RunContent::CommentEnd(id) => m.entry(id).or_default()[1] = at,
                RunContent::CommentRef(id) => m.entry(id).or_default()[2] = at,
                _ => {}
            }
            off += para::atom_len(&item);
        }
    }
    m
}

/// Text of the main story between two positions (for quotes).
fn quote(doc: &Document, a: Pos, b: Pos) -> String {
    let paths = body_paragraphs(doc);
    let mut s = String::new();
    for id in a.para..=b.para {
        let Some(p) = paths.get(id).and_then(|path| para_at(doc, path)) else {
            continue;
        };
        let atoms = para::flatten(p);
        let l = para::len(&atoms);
        let x = if id == a.para { a.offset } else { 0 };
        let y = if id == b.para { b.offset } else { l };
        for (props, item) in para::slice(&atoms, x, y.max(x)) {
            if props.del.is_none() {
                item.push_text(&mut s);
            }
        }
        if id < b.para {
            s.push(' ');
        }
        if s.chars().count() > 120 {
            break;
        }
    }
    s.chars().take(120).collect()
}

/// All comments in part order.
pub fn list(doc: &Document) -> Vec<CommentInfo> {
    if doc.comments.is_empty() {
        return Vec::new();
    }
    let anchors = anchors(doc);
    doc.comments
        .iter()
        .map(|c| {
            let a = anchors.get(&c.id).copied().unwrap_or_default();
            let start = a[0].or(a[2]);
            let end = a[1].or(a[2]).or(start);
            CommentInfo {
                id: c.id,
                author: c.author.clone(),
                initials: c.initials.clone(),
                date: c.date.clone(),
                text: c.text(),
                done: c.done,
                parent: c.parent_para_id.as_ref().and_then(|pid| {
                    doc.comments
                        .iter()
                        .find(|x| x.para_id.as_ref() == Some(pid))
                        .map(|x| x.id)
                }),
                start,
                end,
                quote: match (start, end) {
                    (Some(s), Some(e)) if s <= e => quote(doc, s, e),
                    _ => String::new(),
                },
            }
        })
        .collect()
}

/// Initials of an author name ("Nguyễn Văn An" → "NVA").
pub fn initials(author: &str) -> String {
    author
        .split_whitespace()
        .filter_map(|w| w.chars().next())
        .flat_map(char::to_uppercase)
        .take(4)
        .collect()
}

/// A new `w14:paraId` (8 hex digits below 0x80000000, [MS-DOCX] ST_LongHexNumber)
/// not used by another comment.
pub(crate) fn new_para_id(doc: &Document, seed: &impl std::hash::Hash) -> String {
    let mut h = bo_model::fingerprint(seed) & 0x7FFF_FFFF;
    loop {
        let id = format!("{h:08X}");
        if !doc
            .comments
            .iter()
            .any(|c| c.para_id.as_deref() == Some(id.as_str()))
        {
            return id;
        }
        h = (h + 1) & 0x7FFF_FFFF;
    }
}

/// Comment content: one paragraph per line, the first starting with the
/// annotation reference mark (§17.13.4.1 annotationRef).
pub(crate) fn content(text: &str) -> Vec<Block> {
    text.replace("\r\n", "\n")
        .split('\n')
        .enumerate()
        .map(|(i, line)| {
            let mut runs = Vec::new();
            if i == 0 {
                runs.push(Run {
                    props: RunProps {
                        style: Some("CommentReference".into()),
                        ..RunProps::default()
                    },
                    content: vec![RunContent::Opaque(Raw::new("<w:annotationRef/>", 0))],
                });
            }
            if !line.is_empty() {
                runs.push(Run::text(line, RunProps::default()));
            }
            Block::Paragraph(Paragraph::new(
                bo_model::ParaProps {
                    style: Some("CommentText".into()),
                    ..bo_model::ParaProps::default()
                },
                runs,
            ))
        })
        .collect()
}

/// Next free comment id.
pub(crate) fn next_id(doc: &Document) -> i32 {
    let anchors = anchors(doc);
    doc.comments
        .iter()
        .map(|c| c.id)
        .chain(anchors.keys().copied())
        .max()
        .map_or(0, |m| m + 1)
}

/// Ids of `id` and of every reply below it.
pub(crate) fn thread_ids(doc: &Document, id: i32) -> Vec<i32> {
    let mut out = vec![id];
    let mut i = 0;
    while i < out.len() {
        let cur = out[i];
        if let Some(pid) = doc
            .comments
            .iter()
            .find(|c| c.id == cur)
            .and_then(|c| c.para_id.clone())
        {
            for c in &doc.comments {
                if c.parent_para_id.as_deref() == Some(pid.as_str()) && !out.contains(&c.id) {
                    out.push(c.id);
                }
            }
        }
        i += 1;
    }
    out
}

/// New comment record.
pub(crate) fn make(
    doc: &Document,
    id: i32,
    author: &str,
    date: Option<&str>,
    text: &str,
    parent: Option<&Comment>,
) -> Comment {
    Comment {
        id,
        author: author.to_owned(),
        initials: Some(initials(author)),
        date: date.map(str::to_owned),
        blocks: content(text),
        para_id: Some(new_para_id(doc, &(id, author, date, text))),
        parent_para_id: parent.and_then(|p| p.para_id.clone()),
        done: false,
        durable_id: None,
    }
}
