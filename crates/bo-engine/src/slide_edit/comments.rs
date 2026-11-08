//! BlueSlide comments ([MS-PPTX] modern comments, `p188:cmLst`): the threads of
//! the current slide, add a thread or a reply, resolve, delete. The comment and
//! authors parts are rewritten as preserved package parts (plus the slide and
//! presentation relationships to them), so every change is an ordinary
//! undoable transaction.

use super::chart::put_part;
use super::ops::{PresFields, SlideOp, Tx};
use super::{refuse, R};
use bo_model::slide::{Presentation, Rel};
use bo_pptx::comments::{
    authors_xml, comments_xml, read_authors, read_comments, CommentAuthor, ModernComment,
    ModernReply, CT_AUTHORS, CT_COMMENTS, REL_AUTHORS, REL_COMMENTS,
};
use serde::Serialize;
use std::hash::{Hash, Hasher};

/// One comment of a thread as the UI shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommentOut {
    /// Comment id.
    pub id: String,
    /// Author display name.
    pub author: String,
    /// Creation time (ISO 8601).
    pub date: String,
    /// Plain text.
    pub text: String,
}

/// A comment thread of the current slide.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadOut {
    /// Thread id (id of its first comment).
    pub id: String,
    /// The thread is resolved.
    pub done: bool,
    /// First comment, then the replies.
    pub comments: Vec<CommentOut>,
}

const AUTHORS_PART: &str = "/ppt/authors.xml";

fn comment_part(pres: &Presentation, slide: usize) -> Option<String> {
    let s = pres.slides.get(slide)?;
    let base = if s.info.part_name.is_empty() {
        format!("/ppt/slides/slide{}.xml", slide + 1)
    } else {
        s.info.part_name.clone()
    };
    s.info
        .rels
        .iter()
        .find(|r| r.rel_type == REL_COMMENTS && !r.external)
        .map(|r| bo_opc::resolve_target(&base, &r.target))
}

fn authors_part(pres: &Presentation) -> Option<String> {
    pres.info
        .rels
        .iter()
        .find(|r| r.rel_type == REL_AUTHORS && !r.external)
        .map(|r| bo_opc::resolve_target("/ppt/presentation.xml", &r.target))
}

fn threads(pres: &Presentation, slide: usize) -> Vec<ModernComment> {
    comment_part(pres, slide)
        .and_then(|p| pres.part_data(&p).map(|d| read_comments(d, &p)))
        .unwrap_or_default()
}

fn authors(pres: &Presentation) -> Vec<CommentAuthor> {
    authors_part(pres)
        .and_then(|p| pres.part_data(&p).map(|d| read_authors(d, &p)))
        .unwrap_or_default()
}

/// Threads of `slide` for the UI.
pub fn comments_out(pres: &Presentation, slide: usize) -> Vec<ThreadOut> {
    let list = threads(pres, slide);
    if list.is_empty() {
        return Vec::new();
    }
    let people = authors(pres);
    let name = |id: &str| {
        people
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.name.clone())
            .unwrap_or_default()
    };
    let out = |r: &ModernReply| CommentOut {
        id: r.id.clone(),
        author: name(&r.author_id),
        date: r.created.clone(),
        text: r.text.clone(),
    };
    list.iter()
        .map(|c| ThreadOut {
            id: c.first.id.clone(),
            done: c.resolved,
            comments: std::iter::once(&c.first)
                .chain(&c.replies)
                .map(out)
                .collect(),
        })
        .collect()
}

/// A GUID-shaped id (`{XXXXXXXX-XXXX-4XXX-8XXX-XXXXXXXXXXXX}`) from `seed`,
/// distinct from `taken`.
fn guid(seed: &impl Hash, taken: &dyn Fn(&str) -> bool) -> String {
    let mut n = 0u64;
    loop {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        seed.hash(&mut h);
        n.hash(&mut h);
        let a = h.finish();
        n.hash(&mut h);
        let b = h.finish();
        let id = format!(
            "{{{:08X}-{:04X}-4{:03X}-8{:03X}-{:012X}}}",
            a >> 32,
            (a >> 16) & 0xFFFF,
            a & 0xFFF,
            b >> 52,
            b & 0xFFFF_FFFF_FFFF
        );
        if !taken(&id) {
            return id;
        }
        n += 1;
    }
}

fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|w| w.chars().next())
        .flat_map(char::to_uppercase)
        .take(3)
        .collect()
}

/// Writes `list` as the comment part of `slide` (adding the part and the
/// slide relationship on first use).
fn store(tx: &mut Tx<'_>, slide: usize, list: &[ModernComment]) -> R<()> {
    let s = &tx.pres.slides[slide];
    let sld_id = s.slide_id;
    let part = if let Some(p) = comment_part(tx.pres, slide) {
        p
    } else {
        let name = format!("/ppt/comments/modernComment_{sld_id:X}_0.xml");
        let id = (1..=s.info.rels.len() + 1)
            .map(|n| format!("rId{n}"))
            .find(|id| s.info.rels.iter().all(|r| &r.id != id))
            .unwrap_or_default();
        let rel = Rel {
            id,
            rel_type: REL_COMMENTS.into(),
            target: format!("../comments/modernComment_{sld_id:X}_0.xml"),
            external: false,
        };
        tx.fields(slide, |f| f.rels.push(rel))?;
        name
    };
    put_part(
        tx,
        &part,
        Some(CT_COMMENTS),
        comments_xml(list, sld_id, 0).into_bytes(),
    )
}

/// Author id of `name`, added to the authors part (and the presentation's
/// relationships) when new.
fn author_id(tx: &mut Tx<'_>, name: &str) -> R<String> {
    let name = if name.trim().is_empty() {
        "BlueOffice"
    } else {
        name.trim()
    };
    let mut people = authors(tx.pres);
    if let Some(a) = people.iter().find(|a| a.name == name) {
        return Ok(a.id.clone());
    }
    let id = {
        let ids: Vec<String> = people.iter().map(|a| a.id.clone()).collect();
        guid(&("author", name), &|g| ids.iter().any(|i| i == g))
    };
    people.push(CommentAuthor {
        id: id.clone(),
        name: name.to_owned(),
        initials: initials(name),
        user_id: name.to_owned(),
        provider_id: "None".into(),
    });
    let part = authors_part(tx.pres).unwrap_or_else(|| AUTHORS_PART.to_owned());
    put_part(
        tx,
        &part,
        Some(CT_AUTHORS),
        authors_xml(&people).into_bytes(),
    )?;
    if authors_part(tx.pres).is_none() {
        let before = PresFields::of(tx.pres);
        let mut after = before.clone();
        let n = tx.pres.info.rels.len() + tx.pres.slides.len() + tx.pres.masters.len() + 1;
        let rid = (1..=n)
            .map(|n| format!("rId{n}"))
            .find(|id| {
                tx.pres.info.rels.iter().all(|r| &r.id != id)
                    && tx.pres.slides.iter().all(|s| &s.pres_rel_id != id)
                    && tx.pres.masters.iter().all(|m| &m.pres_rel_id != id)
            })
            .unwrap_or_default();
        after.rels.push(Rel {
            id: rid,
            rel_type: REL_AUTHORS.into(),
            target: "authors.xml".into(),
            external: false,
        });
        tx.op(SlideOp::Pres {
            before: Box::new(before),
            after: Box::new(after),
        })?;
    }
    Ok(id)
}

/// New thread on `slide` (`parent = None`) or a reply to thread `parent`.
pub fn cmd_add(
    tx: &mut Tx<'_>,
    slide: usize,
    text: &str,
    author: &str,
    date: Option<&str>,
    parent: Option<&str>,
) -> R<()> {
    let text = text.trim();
    if text.is_empty() || slide >= tx.pres.slides.len() {
        return refuse("emptyComment");
    }
    let mut list = threads(tx.pres, slide);
    let author_id = author_id(tx, author)?;
    let taken = |g: &str| {
        list.iter()
            .any(|c| c.first.id == g || c.replies.iter().any(|r| r.id == g))
    };
    let n = list.iter().map(|c| c.replies.len() + 1).sum::<usize>();
    let id = guid(&(slide, text, n, parent), &taken);
    let reply = ModernReply {
        id,
        author_id,
        // `created` is required (xsd:dateTime); the UI sends the local clock.
        created: date
            .filter(|d| !d.is_empty())
            .unwrap_or("2000-01-01T00:00:00.000Z")
            .to_owned(),
        text: text.to_owned(),
    };
    match parent {
        Some(p) => {
            let Some(c) = list.iter_mut().find(|c| c.first.id == p) else {
                return refuse("noComment");
            };
            c.replies.push(reply);
        }
        None => list.push(ModernComment {
            first: reply,
            resolved: false,
            replies: Vec::new(),
        }),
    }
    store(tx, slide, &list)
}

/// Resolve (`done`) or reopen a thread.
pub fn cmd_resolve(tx: &mut Tx<'_>, slide: usize, id: &str, done: bool) -> R<()> {
    let mut list = threads(tx.pres, slide);
    let Some(c) = list.iter_mut().find(|c| c.first.id == id) else {
        return refuse("noComment");
    };
    c.resolved = done;
    store(tx, slide, &list)
}

/// Delete a thread (with its replies) or one reply.
pub fn cmd_delete(tx: &mut Tx<'_>, slide: usize, id: &str) -> R<()> {
    let mut list = threads(tx.pres, slide);
    let before = list.len() + list.iter().map(|c| c.replies.len()).sum::<usize>();
    list.retain(|c| c.first.id != id);
    for c in &mut list {
        c.replies.retain(|r| r.id != id);
    }
    if list.len() + list.iter().map(|c| c.replies.len()).sum::<usize>() == before {
        return refuse("noComment");
    }
    store(tx, slide, &list)
}
