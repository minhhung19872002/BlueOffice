//! BlueSheet threaded comments ([MS-XLSX] threaded comments and person parts):
//! list, add a thread, reply, resolve, delete. Each change is one undo step.
//! A new thread also gets the legacy note (ECMA-376 Part 1, §18.7.3 comment)
//! that readers without threaded comments show; its author is `tc=<id>`.

use super::{SheetEngine, UndoEntry};
use crate::{EngineError, Result};
use bo_model::mention::{valid_mentions, CommentTask, Mention, MentionPerson};
use bo_model::sheet::{
    CellRef, Comment, Person, TextRun, ThreadedComment, ThreadedMention, Workbook,
};
use serde::Serialize;
use std::hash::{Hash, Hasher};

/// One comment of a thread as the UI shows it.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CommentView {
    pub id: String,
    pub author: String,
    pub date: String,
    pub text: String,
    /// @mentions in `text` ([MS-XLSX] `mentions`; UTF-16 ranges).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mentions: Vec<Mention>,
    /// Task assigned in the comment (BlueOffice extension).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task: Option<CommentTask>,
}

/// A thread anchored on a cell.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ThreadView {
    pub id: String,
    pub row: u32,
    pub col: u32,
    /// A1 reference of the anchor cell.
    pub cell: String,
    pub done: bool,
    /// First comment, then the replies in file order.
    pub comments: Vec<CommentView>,
}

/// A GUID-shaped id (`{XXXXXXXX-XXXX-4XXX-8XXX-XXXXXXXXXXXX}`, [MS-XLSX] ST_Guid)
/// derived from `seed`, distinct from every id in `taken`.
pub(super) fn guid(seed: &impl Hash, taken: &dyn Fn(&str) -> bool) -> String {
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

fn person_id(wb: &mut Workbook, author: &str) -> String {
    let name = if author.trim().is_empty() {
        "BlueOffice"
    } else {
        author.trim()
    };
    if let Some(p) = wb.persons.iter().find(|p| p.display_name == name) {
        return p.id.clone();
    }
    let ids: Vec<String> = wb.persons.iter().map(|p| p.id.clone()).collect();
    let id = guid(&("person", name), &|g| ids.iter().any(|i| i == g));
    wb.persons.push(Person {
        id: id.clone(),
        display_name: name.to_owned(),
        user_id: Some(name.to_owned()),
        provider_id: Some("None".into()),
    });
    id
}

/// Person of a mention: an existing [MS-XLSX] person with the same user id (e-mail
/// or host id) or name, or a new one (`providerId` "None").
fn mention_person_id(wb: &mut Workbook, p: &MentionPerson) -> String {
    let user = p
        .email
        .clone()
        .filter(|e| !e.is_empty())
        .unwrap_or_else(|| p.id.clone());
    if let Some(x) = wb.persons.iter().find(|x| {
        (!user.is_empty() && x.user_id.as_deref() == Some(user.as_str()))
            || (x.display_name == p.name && x.user_id.is_none())
    }) {
        return x.id.clone();
    }
    let ids: Vec<String> = wb.persons.iter().map(|x| x.id.clone()).collect();
    let id = guid(&("mention", &p.name, &user), &|g| {
        ids.iter().any(|i| i == g)
    });
    wb.persons.push(Person {
        id: id.clone(),
        display_name: p.name.clone(),
        user_id: (!user.is_empty()).then_some(user),
        provider_id: Some("None".into()),
    });
    id
}

/// The UI view of a threaded comment's mentions.
fn mentions_view(wb: &Workbook, c: &ThreadedComment) -> Vec<Mention> {
    c.mentions
        .iter()
        .map(|m| {
            let p = wb.persons.iter().find(|p| p.id == m.person_id);
            let user = p.and_then(|p| p.user_id.clone());
            Mention {
                person: MentionPerson {
                    id: user
                        .clone()
                        .filter(|u| !u.contains('@'))
                        .unwrap_or_default(),
                    name: p.map(|p| p.display_name.clone()).unwrap_or_default(),
                    email: user.filter(|u| u.contains('@')),
                },
                start: m.start,
                length: m.length,
            }
        })
        .collect()
}

impl SheetEngine {
    /// Threads of a sheet in file order of their first comment.
    pub fn comment_threads(&self, sheet: usize) -> Result<Vec<ThreadView>> {
        let wb = self.wb()?;
        let ws = self.sheet(sheet)?;
        let author = |pid: &str| {
            wb.persons
                .iter()
                .find(|p| p.id == pid)
                .map(|p| p.display_name.clone())
                .unwrap_or_default()
        };
        let view = |c: &ThreadedComment| CommentView {
            id: c.id.clone(),
            author: author(&c.person_id),
            date: c.date.clone().unwrap_or_default(),
            text: c.text.clone(),
            mentions: mentions_view(wb, c),
            task: c.task.clone(),
        };
        let mut out: Vec<ThreadView> = ws
            .threaded_comments
            .iter()
            .filter(|c| c.parent_id.is_none())
            .map(|c| ThreadView {
                id: c.id.clone(),
                row: c.cell.row,
                col: c.cell.col,
                cell: c.cell.to_string(),
                done: c.done,
                comments: vec![view(c)],
            })
            .collect();
        for c in &ws.threaded_comments {
            if let Some(t) = c
                .parent_id
                .as_ref()
                .and_then(|p| out.iter_mut().find(|t| &t.id == p))
            {
                t.comments.push(view(c));
            }
        }
        Ok(out)
    }

    fn comment_edit(
        &mut self,
        sheet: usize,
        f: impl FnOnce(&mut Workbook) -> Result<String>,
    ) -> Result<String> {
        self.sheet(sheet)?;
        let wb = self.wb.as_mut().ok_or(EngineError::NoDocument)?;
        let before = Box::new(wb.clone());
        let id = f(wb)?;
        self.push_undo(UndoEntry::Workbook(before));
        Ok(id)
    }

    /// New thread on a cell (`parent = None`) or a reply to thread `parent`.
    /// Returns the new comment's id.
    pub fn add_comment(
        &mut self,
        sheet: usize,
        cell: CellRef,
        text: &str,
        author: &str,
        date: Option<&str>,
        parent: Option<&str>,
    ) -> Result<String> {
        self.add_comment_with(sheet, cell, text, author, date, parent, &[], None)
    }

    /// [`Self::add_comment`] with @mentions (UTF-16 ranges of `text`, stored as
    /// [MS-XLSX] `mentions` + persons) and an assigned task (CORE-COMMENT-001).
    #[allow(clippy::too_many_arguments)]
    pub fn add_comment_with(
        &mut self,
        sheet: usize,
        cell: CellRef,
        text: &str,
        author: &str,
        date: Option<&str>,
        parent: Option<&str>,
        mentions: &[Mention],
        task: Option<CommentTask>,
    ) -> Result<String> {
        // Ranges index the text as sent; trimming shifts them by the leading blanks.
        let lead = bo_model::mention::utf16_len(&text[..text.len() - text.trim_start().len()]);
        let text = text.trim().to_owned();
        let mentions: Vec<Mention> = mentions
            .iter()
            .filter(|m| m.start >= lead)
            .map(|m| Mention {
                start: m.start - lead,
                ..m.clone()
            })
            .collect();
        let mentions = valid_mentions(&text, &mentions);
        if text.is_empty() {
            return Err(EngineError::Sheet("empty comment".into()));
        }
        self.comment_edit(sheet, |wb| {
            let person_id = person_id(wb, author);
            let people: Vec<(String, u32, u32)> = mentions
                .iter()
                .map(|m| (mention_person_id(wb, &m.person), m.start, m.length))
                .collect();
            let ws = &mut wb.sheets[sheet];
            let (cell, parent_id) = if let Some(p) = parent {
                let root = ws
                    .threaded_comments
                    .iter()
                    .find(|c| c.id == p && c.parent_id.is_none())
                    .ok_or_else(|| EngineError::Sheet(format!("no thread {p}")))?;
                (root.cell, Some(p.to_owned()))
            } else {
                if ws
                    .threaded_comments
                    .iter()
                    .any(|c| c.cell == cell && c.parent_id.is_none())
                {
                    return Err(EngineError::Sheet(format!("{cell} already has a thread")));
                }
                (cell, None)
            };
            let n = ws.threaded_comments.len();
            let id = {
                let list = &ws.threaded_comments;
                guid(&(sheet, cell.row, cell.col, &text, n), &|g| {
                    list.iter().any(|c| c.id == g)
                })
            };
            let c = ThreadedComment {
                id: id.clone(),
                cell,
                person_id,
                date: date.filter(|d| !d.is_empty()).map(str::to_owned),
                parent_id: parent_id.clone(),
                text: text.clone(),
                done: false,
                mentions: people
                    .iter()
                    .enumerate()
                    .map(|(k, (pid, start, length))| ThreadedMention {
                        person_id: pid.clone(),
                        mention_id: guid(&(&id, k, pid), &|g| g == id),
                        start: *start,
                        length: *length,
                    })
                    .collect(),
                task,
            };
            if let Some(p) = &parent_id {
                // A reply goes after the last comment of its thread.
                let at = ws
                    .threaded_comments
                    .iter()
                    .rposition(|x| x.id == *p || x.parent_id.as_ref() == Some(p))
                    .map_or(ws.threaded_comments.len(), |i| i + 1);
                ws.threaded_comments.insert(at, c);
            } else {
                ws.threaded_comments.push(c);
                // §18.7.3 legacy note for readers without threaded comments.
                ws.comments.retain(|n| n.cell != cell);
                ws.comments.push(Comment {
                    cell,
                    author: format!("tc={id}"),
                    text: vec![TextRun {
                        text: text.clone(),
                        font: None,
                    }],
                });
            }
            Ok(id)
        })
    }

    /// Resolves (`done`) or reopens a thread.
    pub fn resolve_comment(&mut self, sheet: usize, id: &str, done: bool) -> Result<()> {
        self.comment_edit(sheet, |wb| {
            let c = wb.sheets[sheet]
                .threaded_comments
                .iter_mut()
                .find(|c| c.id == id && c.parent_id.is_none())
                .ok_or_else(|| EngineError::Sheet(format!("no thread {id}")))?;
            c.done = done;
            Ok(String::new())
        })
        .map(|_| ())
    }

    /// Deletes a thread with its replies (or one reply) and the thread's legacy note.
    pub fn delete_comment(&mut self, sheet: usize, id: &str) -> Result<()> {
        self.comment_edit(sheet, |wb| {
            let ws = &mut wb.sheets[sheet];
            let target = ws
                .threaded_comments
                .iter()
                .find(|c| c.id == id)
                .cloned()
                .ok_or_else(|| EngineError::Sheet(format!("no comment {id}")))?;
            if target.parent_id.is_none() {
                ws.threaded_comments
                    .retain(|c| c.id != id && c.parent_id.as_deref() != Some(id));
                let tc = format!("tc={id}");
                ws.comments.retain(|n| {
                    !(n.cell == target.cell && (n.author == tc || n.author.starts_with("tc=")))
                });
            } else {
                ws.threaded_comments.retain(|c| c.id != id);
            }
            Ok(String::new())
        })
        .map(|_| ())
    }
}
