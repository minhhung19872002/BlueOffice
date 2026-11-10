//! Comments with replies (ECMA-376 Part 1, §17.13.4; threading per [MS-DOCX]
//! §2.5.2 `w15:commentEx`): listing threads with their anchors in the main text
//! and the content of new comments.

use crate::para;
use crate::Pos;
use bo_model::index::{body_paragraphs, para_at};
use bo_model::mention::{
    mailto, utf16_len, utf16_range, valid_mentions, CommentTask, Mention, MentionPerson,
};
use bo_model::{Block, Comment, Document, Paragraph, Raw, Run, RunContent, RunProps};
use serde::Serialize;

/// A comment for the comments pane (threads are rebuilt from `parent`).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentInfo {
    /// Comment id (`w:comment @w:id`).
    pub id: i32,
    /// Author name.
    pub author: String,
    /// Author initials, if set.
    pub initials: Option<String>,
    /// Creation date (`ST_DateTime`), if set.
    pub date: Option<String>,
    /// Plain text of the comment (paragraphs joined by `
    /// `).
    pub text: String,
    /// The comment is marked as resolved.
    pub done: bool,
    /// Id of the comment this one replies to.
    pub parent: Option<i32>,
    /// Anchor range in the main text (missing for comments without range marks).
    pub start: Option<Pos>,
    /// End of the anchor range.
    pub end: Option<Pos>,
    /// Commented text (shortened).
    pub quote: String,
    /// @mentions in `text` (`mailto:` hyperlinks "@Name", UTF-16 ranges).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<Mention>,
    /// Task assigned in the comment (complete when the thread is resolved).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<CommentTask>,
}

/// Mentions of a comment: `mailto:` hyperlink runs "@Name", located in its text.
pub(crate) fn mentions_of(c: &Comment, text: &str) -> Vec<Mention> {
    let mut out = Vec::new();
    let mut from = 0usize;
    for b in &c.blocks {
        let Block::Paragraph(p) = b else { continue };
        let mut i = 0;
        while i < p.runs.len() {
            let link = p.runs[i].props.link.clone();
            // Consecutive runs of the same hyperlink form one mention.
            let mut label = String::new();
            let mut j = i;
            while j < p.runs.len() && p.runs[j].props.link == link {
                label.push_str(&p.runs[j].plain_text());
                j += 1;
            }
            i = j;
            let Some(email) = link.as_deref().and_then(mailto) else {
                continue;
            };
            let Some(name) = label.strip_prefix('@') else {
                continue;
            };
            let Some(at) = text[from..].find(&label).map(|k| k + from) else {
                continue;
            };
            from = at + label.len();
            out.push(Mention {
                person: MentionPerson {
                    id: String::new(),
                    name: name.trim().to_owned(),
                    email: Some(email),
                },
                start: utf16_len(&text[..at]),
                length: utf16_len(&label),
            });
        }
    }
    out
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
            let text = c.text();
            CommentInfo {
                id: c.id,
                author: c.author.clone(),
                initials: c.initials.clone(),
                date: c.date.clone(),
                mentions: mentions_of(c, &text),
                task: c.task.clone(),
                text,
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

/// Runs of one line of a new comment: plain text, and each mention with an
/// e-mail address as a `mailto:` hyperlink (§17.16.22) "@Name".
fn line_runs(line: &str, at: usize, mentions: &[(std::ops::Range<usize>, &Mention)]) -> Vec<Run> {
    let mut runs = Vec::new();
    let mut pos = 0;
    for (r, m) in mentions {
        if r.start < at || r.end > at + line.len() {
            continue;
        }
        let (a, b) = (r.start - at, r.end - at);
        if a > pos {
            runs.push(Run::text(&line[pos..a], RunProps::default()));
        }
        let props = RunProps {
            link: m.person.email.as_ref().map(|e| format!("mailto:{e}")),
            ..RunProps::default()
        };
        runs.push(Run::text(&line[a..b], props));
        pos = b;
    }
    if pos < line.len() {
        runs.push(Run::text(&line[pos..], RunProps::default()));
    }
    runs
}

/// Comment content: one paragraph per line, the first starting with the
/// annotation reference mark (§17.13.4.1 annotationRef); `mentions` index the
/// text in UTF-16 code units.
pub(crate) fn content(text: &str, mentions: &[Mention]) -> Vec<Block> {
    let text = text.replace("\r\n", "\n");
    let valid = valid_mentions(&text, mentions);
    let ranges: Vec<(std::ops::Range<usize>, &Mention)> = valid
        .iter()
        .filter_map(|m| Some((utf16_range(&text, m.start, m.length)?, m)))
        .collect();
    let mut at = 0;
    text.split('\n')
        .enumerate()
        .map(|(i, line)| {
            let start = at;
            at += line.len() + 1;
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
            runs.extend(line_runs(line, start, &ranges));
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
#[allow(clippy::too_many_arguments)]
pub(crate) fn make(
    doc: &Document,
    id: i32,
    author: &str,
    date: Option<&str>,
    text: &str,
    parent: Option<&Comment>,
    mentions: &[Mention],
    task: Option<CommentTask>,
) -> Comment {
    Comment {
        id,
        author: author.to_owned(),
        initials: Some(initials(author)),
        date: date.map(str::to_owned),
        blocks: content(text, mentions),
        para_id: Some(new_para_id(doc, &(id, author, date, text))),
        parent_para_id: parent.and_then(|p| p.para_id.clone()),
        done: false,
        durable_id: None,
        task,
    }
}
