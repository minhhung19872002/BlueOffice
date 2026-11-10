//! Editing an equation in place (`docs/parity/BACKLOG.md` CORE-MATH-010, ADR-0015): positions inside the
//! [`Node`] tree, caret motion through the slots of its structures, typing, deleting and the linear-format
//! *autobuild* (typed UnicodeMath such as `x^2` built up into the structure when a space follows).
//!
//! A *slot* is a list of nodes: the zone itself (`m:oMath`, ECMA-376 Part 1, §22.1.2.77) or an argument
//! of a structure (`m:num`, `m:den`, `m:e`, `m:sub`, `m:sup`, `m:deg`, `m:lim`, `m:fName`, §22.1.2).
//! A [`MathPos`] is a path of [`Step`]s from the zone down to a slot and an offset in that slot,
//! counted in characters of its runs (`m:r`, §22.1.2.98) plus one per structure.
//!
//! The slots of a structure are numbered in reading order (see [`slots`]); every slot of a zone in
//! pre-order (a slot, then the slots of the structures in it) is [`all_slots`], the order in which the
//! written OMML lists them — layout reports slot geometry in that order.

use crate::unicode_math::{expand_control_words, merge, parse};
use crate::{Node, Pr, RunPr, Style, XmlElement};

/// One step down the tree: structure `item` of the current slot, its slot `slot`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Step {
    /// Index of the structure among the nodes of the slot.
    pub item: usize,
    /// Index of the slot among the structure's slots ([`slots`]).
    pub slot: usize,
}

/// A caret position inside an equation.
#[derive(Debug, Clone, PartialEq, Eq, Default, Hash)]
pub struct MathPos {
    /// From the zone down to the slot (empty: the zone itself).
    pub path: Vec<Step>,
    /// Offset in the slot: characters of runs, one per structure.
    pub offset: usize,
}

impl MathPos {
    /// Offset `offset` of the slot at `path`.
    #[must_use]
    pub fn new(path: Vec<Step>, offset: usize) -> MathPos {
        MathPos { path, offset }
    }
}

/// The slots of a structure in reading order: fraction num, den (§22.1.2.36); scripts base, sub, sup
/// (§22.1.2.110–112); radical degree, radicand (§22.1.2.88); n-ary lower limit, upper limit, operand
/// (§22.1.2.70); delimiter parts (§22.1.2.24); function name, argument (§22.1.2.39); limit base, limit
/// (§22.1.2.54/56); matrix cells row by row (§22.1.2.60); equation array rows (§22.1.2.34); accent and
/// bar base (§22.1.2.1, §22.1.2.7); the base of a grouping character, box, border box and phantom; the
/// subscript, superscript and base of pre-scripts (§22.1.2 groupChr, box, borderBox, phant, sPre). A run
/// has none.
#[must_use]
pub fn slots(n: &Node) -> Vec<&Vec<Node>> {
    match n {
        Node::Run(..) => Vec::new(),
        Node::Frac { num, den, .. } => vec![num, den],
        Node::Scripts { base, sub, sup, .. } => std::iter::once(base)
            .chain(sub.iter())
            .chain(sup.iter())
            .collect(),
        Node::Rad { deg, body, .. } => deg.iter().chain(std::iter::once(body)).collect(),
        Node::Nary { sub, sup, body, .. } => sub
            .iter()
            .chain(sup.iter())
            .chain(std::iter::once(body))
            .collect(),
        Node::Delim { parts, .. } => parts.iter().collect(),
        Node::Func { name, arg, .. } => vec![name, arg],
        Node::Limit { base, lim, .. } => vec![base, lim],
        Node::Matrix(rows, _) => rows.iter().flatten().collect(),
        Node::EqArray(rows, _) => rows.iter().collect(),
        Node::Accent { base, .. }
        | Node::Bar { base, .. }
        | Node::GroupChr { base, .. }
        | Node::Boxed { base, .. }
        | Node::BorderBox { base, .. }
        | Node::Phantom { base, .. } => vec![base],
        Node::PreScripts { sub, sup, base, .. } => vec![sub, sup, base],
    }
}

/// [`slots`], mutable.
pub fn slots_mut(n: &mut Node) -> Vec<&mut Vec<Node>> {
    match n {
        Node::Run(..) => Vec::new(),
        Node::Frac { num, den, .. } => vec![num, den],
        Node::Scripts { base, sub, sup, .. } => std::iter::once(base)
            .chain(sub.iter_mut())
            .chain(sup.iter_mut())
            .collect(),
        Node::Rad { deg, body, .. } => deg.iter_mut().chain(std::iter::once(body)).collect(),
        Node::Nary { sub, sup, body, .. } => sub
            .iter_mut()
            .chain(sup.iter_mut())
            .chain(std::iter::once(body))
            .collect(),
        Node::Delim { parts, .. } => parts.iter_mut().collect(),
        Node::Func { name, arg, .. } => vec![name, arg],
        Node::Limit { base, lim, .. } => vec![base, lim],
        Node::Matrix(rows, _) => rows.iter_mut().flatten().collect(),
        Node::EqArray(rows, _) => rows.iter_mut().collect(),
        Node::Accent { base, .. }
        | Node::Bar { base, .. }
        | Node::GroupChr { base, .. }
        | Node::Boxed { base, .. }
        | Node::BorderBox { base, .. }
        | Node::Phantom { base, .. } => vec![base],
        Node::PreScripts { sub, sup, base, .. } => vec![sub, sup, base],
    }
}

/// Length of a node in a slot: the characters of a run, 1 for a structure.
#[must_use]
pub fn item_len(n: &Node) -> usize {
    match n {
        Node::Run(t, ..) => t.chars().count(),
        _ => 1,
    }
}

/// Length of a slot.
#[must_use]
pub fn slot_len(s: &[Node]) -> usize {
    s.iter().map(item_len).sum()
}

/// Offset at which item `i` of a slot starts.
#[must_use]
pub fn item_start(s: &[Node], i: usize) -> usize {
    s.iter().take(i).map(item_len).sum()
}

/// The slot at `path`.
#[must_use]
pub fn slot_at<'a>(root: &'a [Node], path: &[Step]) -> Option<&'a [Node]> {
    let mut s = root;
    for st in path {
        let n = s.get(st.item)?;
        s = slots(n).get(st.slot).copied()?.as_slice();
    }
    Some(s)
}

/// The slot at `path`, mutable.
pub fn slot_mut<'a>(root: &'a mut Vec<Node>, path: &[Step]) -> Option<&'a mut Vec<Node>> {
    let mut s = root;
    for st in path {
        let n = s.get_mut(st.item)?;
        s = slots_mut(n).into_iter().nth(st.slot)?;
    }
    Some(s)
}

/// What is next to an offset of a slot: (item, `Some(char index)` in a run or `None` for a structure).
fn after(s: &[Node], off: usize) -> Option<(usize, Option<usize>)> {
    let mut at = 0;
    for (i, n) in s.iter().enumerate() {
        let l = item_len(n);
        if off >= at && off < at + l {
            return Some((i, matches!(n, Node::Run(..)).then_some(off - at)));
        }
        at += l;
    }
    None
}

fn before(s: &[Node], off: usize) -> Option<(usize, Option<usize>)> {
    let mut at = 0;
    for (i, n) in s.iter().enumerate() {
        let l = item_len(n);
        if off > at && off <= at + l {
            return Some((i, matches!(n, Node::Run(..)).then_some(off - at - 1)));
        }
        at += l;
    }
    None
}

/// `pos` exists in the tree.
#[must_use]
pub fn is_valid(root: &[Node], pos: &MathPos) -> bool {
    slot_at(root, &pos.path).is_some_and(|s| pos.offset <= slot_len(s))
}

/// `pos` made valid for `root` (after the tree changed under it): the deepest slot of the path that
/// still exists, the offset clamped; a step that no longer exists leaves the caret before that item.
#[must_use]
pub fn clamp(root: &[Node], pos: &MathPos) -> MathPos {
    let mut s = root;
    let mut path = Vec::new();
    for st in &pos.path {
        let next = s.get(st.item).and_then(|n| slots(n).get(st.slot).copied());
        let Some(v) = next else {
            let off = item_start(s, st.item.min(s.len()));
            return MathPos::new(path, off);
        };
        path.push(*st);
        s = v;
    }
    MathPos::new(path, pos.offset.min(slot_len(s)))
}

/// Every slot of the zone in pre-order (the zone first, then the slots of each structure in reading
/// order, each followed by the slots inside it).
#[must_use]
pub fn all_slots(root: &[Node]) -> Vec<Vec<Step>> {
    fn walk(s: &[Node], path: &mut Vec<Step>, out: &mut Vec<Vec<Step>>) {
        out.push(path.clone());
        for (i, n) in s.iter().enumerate() {
            for (k, sl) in slots(n).iter().enumerate() {
                path.push(Step { item: i, slot: k });
                walk(sl, path, out);
                path.pop();
            }
        }
    }
    let mut out = Vec::new();
    walk(root, &mut Vec::new(), &mut out);
    out
}

/// Index of the slot at `path` in [`all_slots`].
#[must_use]
pub fn slot_index(root: &[Node], path: &[Step]) -> Option<usize> {
    all_slots(root).iter().position(|p| p == path)
}

/// One position to the right in reading order; `None` past the end of the zone. A structure is entered
/// at the start of its first slot; the end of a slot goes to the start of the next slot of the
/// structure, the end of its last slot to just after the structure.
#[must_use]
pub fn right(root: &[Node], pos: &MathPos) -> Option<MathPos> {
    let s = slot_at(root, &pos.path)?;
    match after(s, pos.offset) {
        Some((_, Some(_))) => Some(MathPos::new(pos.path.clone(), pos.offset + 1)),
        Some((i, None)) => {
            if slots(&s[i]).is_empty() {
                return Some(MathPos::new(pos.path.clone(), pos.offset + 1));
            }
            let mut path = pos.path.clone();
            path.push(Step { item: i, slot: 0 });
            Some(MathPos::new(path, 0))
        }
        None => exit_forward(root, &pos.path),
    }
}

/// From the end of the slot at `path`: the next slot of its structure, else after the structure.
fn exit_forward(root: &[Node], path: &[Step]) -> Option<MathPos> {
    let (last, parent) = path.split_last()?;
    let ps = slot_at(root, parent)?;
    let n = ps.get(last.item)?;
    if last.slot + 1 < slots(n).len() {
        let mut p = parent.to_vec();
        p.push(Step {
            item: last.item,
            slot: last.slot + 1,
        });
        return Some(MathPos::new(p, 0));
    }
    Some(MathPos::new(parent.to_vec(), item_start(ps, last.item) + 1))
}

/// From the start of the slot at `path`: the end of the previous slot of its structure, else before it.
fn exit_backward(root: &[Node], path: &[Step]) -> Option<MathPos> {
    let (last, parent) = path.split_last()?;
    let ps = slot_at(root, parent)?;
    let n = ps.get(last.item)?;
    if last.slot > 0 {
        let prev = slots(n).get(last.slot - 1).map_or(0, |v| slot_len(v));
        let mut p = parent.to_vec();
        p.push(Step {
            item: last.item,
            slot: last.slot - 1,
        });
        return Some(MathPos::new(p, prev));
    }
    Some(MathPos::new(parent.to_vec(), item_start(ps, last.item)))
}

/// One position to the left in reading order; `None` before the start of the zone.
#[must_use]
pub fn left(root: &[Node], pos: &MathPos) -> Option<MathPos> {
    let s = slot_at(root, &pos.path)?;
    match before(s, pos.offset) {
        Some((_, Some(_))) => Some(MathPos::new(pos.path.clone(), pos.offset - 1)),
        Some((i, None)) => {
            let sl = slots(&s[i]);
            let Some(last) = sl.last() else {
                return Some(MathPos::new(pos.path.clone(), pos.offset - 1));
            };
            let mut path = pos.path.clone();
            path.push(Step {
                item: i,
                slot: sl.len() - 1,
            });
            Some(MathPos::new(path, slot_len(last)))
        }
        None => exit_backward(root, &pos.path),
    }
}

/// Tab (`back`: Shift+Tab): the next slot — the first slot of the next structure after the caret in
/// this slot, else the next slot of the enclosing structure, else just after it; the zone keeps the
/// caret when nothing follows. Shift+Tab mirrors it (the end of the previous slot).
#[must_use]
pub fn tab(root: &[Node], pos: &MathPos, back: bool) -> MathPos {
    let Some(s) = slot_at(root, &pos.path) else {
        return clamp(root, pos);
    };
    let mut at = 0;
    let mut found = None;
    for (i, n) in s.iter().enumerate() {
        let l = item_len(n);
        let ns = slots(n);
        if !ns.is_empty() {
            if !back && at >= pos.offset && found.is_none() {
                found = Some((i, 0, 0));
            }
            if back && at + l <= pos.offset {
                let k = ns.len() - 1;
                found = Some((i, k, slot_len(ns[k])));
            }
        }
        at += l;
    }
    if let Some((i, k, off)) = found {
        let mut path = pos.path.clone();
        path.push(Step { item: i, slot: k });
        return MathPos::new(path, off);
    }
    let out = if back {
        exit_backward(root, &pos.path)
    } else {
        exit_forward(root, &pos.path)
    };
    out.unwrap_or_else(|| pos.clone())
}

/// Start (`end`: end) of the zone.
#[must_use]
pub fn zone_end(root: &[Node], end: bool) -> MathPos {
    MathPos::new(Vec::new(), if end { slot_len(root) } else { 0 })
}

/// Insert `text` at `pos` as math text: into the run at the caret (the one before it at a boundary),
/// else as a new run. Returns the caret after the text.
pub fn insert_text(root: &mut Vec<Node>, pos: &MathPos, text: &str) -> Option<MathPos> {
    let fmt = ambient(root, pos);
    let s = slot_mut(root, &pos.path)?;
    if pos.offset > slot_len(s) {
        return None;
    }
    let n = text.chars().count();
    let mut at = 0;
    let mut target = None;
    let mut index = s.len();
    for (i, item) in s.iter().enumerate() {
        let l = item_len(item);
        if let Node::Run(..) = item {
            if pos.offset >= at && pos.offset <= at + l {
                target = Some((i, pos.offset - at));
                break;
            }
        }
        if at >= pos.offset {
            index = i;
            break;
        }
        at += l;
    }
    match target {
        Some((i, c)) => {
            if let Some(Node::Run(t, ..)) = s.get_mut(i) {
                let b = t.char_indices().nth(c).map_or(t.len(), |(b, _)| b);
                t.insert_str(b, text);
            }
        }
        None => s.insert(
            index,
            Node::Run(text.to_owned(), Style::Math, RunPr::text_only(fmt)),
        ),
    }
    Some(MathPos::new(pos.path.clone(), pos.offset + n))
}

/// The text formatting (`w:rPr`, ECMA-376 Part 1, §17.3.2.28) for a run typed at `pos` where no run
/// is next to the caret: that of the nearest run of the slot, else the control properties of the
/// enclosing structure (§22.1.2 ctrlPr), else of the slots around it, else of any run of the zone —
/// so the math font of the document carries over.
#[must_use]
pub fn ambient(root: &[Node], pos: &MathPos) -> Option<XmlElement> {
    fn fmt(n: &Node) -> Option<&XmlElement> {
        match n {
            Node::Run(_, _, p) => p.text.as_ref(),
            _ => None,
        }
    }
    fn run_fmt(s: &[Node], before: usize) -> Option<&XmlElement> {
        let split = before.min(s.len());
        s[..split]
            .iter()
            .rev()
            .find_map(fmt)
            .or_else(|| s[split..].iter().find_map(fmt))
    }
    fn any(s: &[Node]) -> Option<&XmlElement> {
        s.iter().find_map(|n| match n {
            Node::Run(_, _, p) => p.text.as_ref(),
            _ => n
                .pr()
                .and_then(crate::Pr::ctrl_rpr)
                .or_else(|| slots(n).into_iter().find_map(|v| any(v))),
        })
    }
    // Item index of the caret in each slot along the path.
    let mut levels: Vec<(&[Node], usize)> = Vec::new();
    let mut s = root;
    for st in &pos.path {
        levels.push((s, st.item));
        s = s
            .get(st.item)
            .and_then(|n| slots(n).get(st.slot).copied())
            .map_or(&[][..], Vec::as_slice);
    }
    let here = after(s, pos.offset).map_or(s.len(), |(i, _)| i);
    if let Some(f) = run_fmt(s, here) {
        return Some(f.clone());
    }
    for (slot, item) in levels.iter().rev() {
        let ctrl = slot
            .get(*item)
            .and_then(Node::pr)
            .and_then(crate::Pr::ctrl_rpr);
        if let Some(f) = ctrl.or_else(|| run_fmt(slot, *item)) {
            return Some(f.clone());
        }
    }
    any(root).cloned()
}

/// Result of Backspace / Delete inside an equation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Deleted {
    /// A character was removed; the caret.
    Char(MathPos),
    /// Nothing removed: the structure just before this caret is to be selected (the first step of
    /// deleting a structure; [`delete_selected`] is the second).
    Select(MathPos),
    /// At the start (Backspace) or end (Delete) of the zone: nothing to do inside it.
    Outside,
}

/// Backspace (`back`) or Delete at `pos`: a character next to the caret goes; next to a structure or at
/// the edge of a slot the structure is selected first ([`Deleted::Select`]).
pub fn delete(root: &mut Vec<Node>, pos: &MathPos, back: bool) -> Option<Deleted> {
    let s = slot_mut(root, &pos.path)?;
    let near = if back {
        before(s, pos.offset)
    } else {
        after(s, pos.offset)
    };
    match near {
        Some((i, Some(c))) => {
            if let Some(Node::Run(t, ..)) = s.get_mut(i) {
                if let Some((b, ch)) = t.char_indices().nth(c) {
                    t.replace_range(b..b + ch.len_utf8(), "");
                }
                if t.is_empty() {
                    s.remove(i);
                }
            }
            let off = if back { pos.offset - 1 } else { pos.offset };
            Some(Deleted::Char(MathPos::new(pos.path.clone(), off)))
        }
        Some((_, None)) => {
            let off = if back { pos.offset } else { pos.offset + 1 };
            Some(Deleted::Select(MathPos::new(pos.path.clone(), off)))
        }
        None => {
            let Some((last, parent)) = pos.path.split_last() else {
                return Some(Deleted::Outside);
            };
            let ps = slot_at(root, parent)?;
            Some(Deleted::Select(MathPos::new(
                parent.to_vec(),
                item_start(ps, last.item) + 1,
            )))
        }
    }
}

/// Remove the structure just before `pos` (a selection made by [`Deleted::Select`]); the caret where it
/// was.
pub fn delete_selected(root: &mut Vec<Node>, pos: &MathPos) -> Option<MathPos> {
    let s = slot_mut(root, &pos.path)?;
    let (i, c) = before(s, pos.offset)?;
    if c.is_some() {
        return None;
    }
    s.remove(i);
    let merged = merge(std::mem::take(s));
    *s = merged;
    Some(MathPos::new(pos.path.clone(), pos.offset - 1))
}

/// The structure just before `pos` (to show a selected structure): its item index in the slot.
#[must_use]
pub fn structure_before(root: &[Node], pos: &MathPos) -> Option<usize> {
    let s = slot_at(root, &pos.path)?;
    match before(s, pos.offset)? {
        (i, None) => Some(i),
        _ => None,
    }
}

/// Path (relative to `nodes`) of the first empty slot in reading order.
fn first_empty(nodes: &[Node]) -> Option<Vec<Step>> {
    for (i, n) in nodes.iter().enumerate() {
        for (k, sl) in slots(n).iter().enumerate() {
            if sl.is_empty() {
                return Some(vec![Step { item: i, slot: k }]);
            }
            if let Some(mut rest) = first_empty(sl) {
                rest.insert(0, Step { item: i, slot: k });
                return Some(rest);
            }
        }
    }
    None
}

/// Stand-in operand for a structure that scripts typed after it apply to (private use character).
const HOLE: char = '\u{E000}';

/// Autobuild (UnicodeMath build-up, after UTN #28 §3.13 "build-up on typing"): the math text typed just
/// before the caret in its run is parsed as UnicodeMath (control words such as `\alpha`, `\sqrt`,
/// `\sum` expanded) and replaced by the structures it denotes — `x^2`, `a/b`, `√x`, `∑`, `(…)`, …
/// A run starting with `^` / `_` right after a structure scripts that structure. Returns the caret
/// (in the first empty slot of what was built, else after it), `None` when nothing builds.
pub fn autobuild(root: &mut Vec<Node>, pos: &MathPos) -> Option<MathPos> {
    let s = slot_mut(root, &pos.path)?;
    let (i, c) = before(s, pos.offset)?;
    let c = c?;
    let Node::Run(t, Style::Math, typed_pr) = &s[i] else {
        return None;
    };
    let typed_pr = typed_pr.clone();
    let split = t.char_indices().nth(c + 1).map_or(t.len(), |(b, _)| b);
    let (typed, rest) = (t[..split].to_owned(), t[split..].to_owned());
    let start = item_start(s, i);
    // Scripts typed after a structure: `□^2` with the structure as the base.
    let scripted = (typed.starts_with('^') || typed.starts_with('_'))
        && i > 0
        && !matches!(s[i - 1], Node::Run(..));
    let expanded = expand_control_words(&typed);
    let mut built = if scripted {
        let mut v = parse(&format!("{HOLE}{expanded}"));
        dress(&mut v, &typed_pr);
        match v.first_mut() {
            Some(Node::Scripts { base, .. }) if matches!(base.as_slice(), [Node::Run(h, ..)] if h == &HOLE.to_string()) =>
            {
                *base = vec![s[i - 1].clone()];
            }
            _ => return None,
        }
        v
    } else {
        let mut v = parse(&expanded);
        dress(&mut v, &typed_pr);
        v
    };
    let structure = built.iter().any(|n| !matches!(n, Node::Run(..)));
    if !structure {
        if expanded == typed {
            return None;
        }
        // Only symbols changed (`\alpha` → α): keep the text as typed otherwise.
        built = vec![Node::Run(expanded, Style::Math, typed_pr.clone())];
    }
    let first = if scripted { i - 1 } else { i };
    let base_off = if scripted { start - 1 } else { start };
    let caret = match first_empty(&built) {
        Some(rel) => {
            let mut path = pos.path.clone();
            let mut rel = rel.into_iter();
            if let Some(f) = rel.next() {
                path.push(Step {
                    item: first + f.item,
                    slot: f.slot,
                });
            }
            path.extend(rel);
            MathPos::new(path, 0)
        }
        None => MathPos::new(pos.path.clone(), base_off + slot_len(&built)),
    };
    let mut tail = built;
    if !rest.is_empty() {
        tail.push(Node::Run(rest, Style::Math, typed_pr));
    }
    s.splice(first..=i, tail);
    Some(caret)
}

/// Formatting of what autobuild made from text typed into a run with `typed`: every run takes that
/// run's formatting (its style set anew, [`RunPr::inherit`]), every structure control properties with
/// its `w:rPr` ([`Pr::with_ctrl`], ECMA-376 Part 1, §22.1.2 ctrlPr), so the math font of the
/// document is kept.
fn dress(nodes: &mut [Node], typed: &RunPr) {
    for n in nodes {
        if let Node::Run(_, _, p) = n {
            *p = typed.inherit();
            continue;
        }
        if let Some(pr) = n.pr_mut() {
            *pr = Pr::with_ctrl(typed.text.as_ref());
        }
        for s in slots_mut(n) {
            dress(s, typed);
        }
    }
}

/// The tree as it reads back after being written: adjacent math runs of every slot joined, empty runs
/// dropped, empty radical degrees and n-ary limits hidden (`degHide`, `subHide`, `supHide`).
pub fn normalize(nodes: &mut Vec<Node>) {
    nodes.retain(|n| !matches!(n, Node::Run(t, ..) if t.is_empty()));
    for n in nodes.iter_mut() {
        match n {
            Node::Rad { deg, .. } if deg.as_ref().is_some_and(Vec::is_empty) => *deg = None,
            Node::Nary { sub, sup, .. } => {
                if sub.as_ref().is_some_and(Vec::is_empty) {
                    *sub = None;
                }
                if sup.as_ref().is_some_and(Vec::is_empty) {
                    *sup = None;
                }
            }
            _ => {}
        }
        for s in slots_mut(n) {
            normalize(s);
        }
    }
    let merged = merge(std::mem::take(nodes));
    *nodes = merged;
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn r(t: &str) -> Node {
        Node::run(t, Style::Math)
    }

    fn p(steps: &[(usize, usize)], offset: usize) -> MathPos {
        MathPos::new(
            steps
                .iter()
                .map(|&(item, slot)| Step { item, slot })
                .collect(),
            offset,
        )
    }

    /// Walks right from the start to the end and back, recording each position.
    fn walk(root: &[Node]) -> Vec<MathPos> {
        let mut out = vec![zone_end(root, false)];
        while let Some(n) = right(root, out.last().unwrap()) {
            assert!(is_valid(root, &n), "{n:?}");
            out.push(n);
            assert!(out.len() < 200);
        }
        // Left retraces the same positions.
        let mut back = vec![out.last().unwrap().clone()];
        while let Some(n) = left(root, back.last().unwrap()) {
            back.push(n);
        }
        back.reverse();
        assert_eq!(back, out);
        out
    }

    // source: reading order of the OMML structures, ECMA-376 Part 1 §22.1.2.
    #[test]
    fn navigation_over_every_structure_kind() {
        let cases: Vec<(&str, Vec<MathPos>)> = vec![
            (
                "a/b",
                vec![
                    p(&[], 0),
                    p(&[(0, 0)], 0),
                    p(&[(0, 0)], 1),
                    p(&[(0, 1)], 0),
                    p(&[(0, 1)], 1),
                    p(&[], 1),
                ],
            ),
            (
                "x_i^2",
                vec![
                    p(&[], 0),
                    p(&[(0, 0)], 0),
                    p(&[(0, 0)], 1),
                    p(&[(0, 1)], 0),
                    p(&[(0, 1)], 1),
                    p(&[(0, 2)], 0),
                    p(&[(0, 2)], 1),
                    p(&[], 1),
                ],
            ),
            (
                "√(n&x)",
                vec![
                    p(&[], 0),
                    p(&[(0, 0)], 0),
                    p(&[(0, 0)], 1),
                    p(&[(0, 1)], 0),
                    p(&[(0, 1)], 1),
                    p(&[], 1),
                ],
            ),
            (
                "∑_k^n k",
                vec![
                    p(&[], 0),
                    p(&[(0, 0)], 0),
                    p(&[(0, 0)], 1),
                    p(&[(0, 1)], 0),
                    p(&[(0, 1)], 1),
                    p(&[(0, 2)], 0),
                    p(&[(0, 2)], 1),
                    p(&[], 1),
                ],
            ),
            (
                "(a)",
                vec![p(&[], 0), p(&[(0, 0)], 0), p(&[(0, 0)], 1), p(&[], 1)],
            ),
            (
                "■(a&b@c&d)",
                vec![
                    p(&[], 0),
                    p(&[(0, 0)], 0),
                    p(&[(0, 0)], 1),
                    p(&[(0, 1)], 0),
                    p(&[(0, 1)], 1),
                    p(&[(0, 2)], 0),
                    p(&[(0, 2)], 1),
                    p(&[(0, 3)], 0),
                    p(&[(0, 3)], 1),
                    p(&[], 1),
                ],
            ),
            (
                "sin x",
                vec![
                    p(&[], 0),
                    p(&[(0, 0)], 0),
                    p(&[(0, 0)], 1),
                    p(&[(0, 0)], 2),
                    p(&[(0, 0)], 3),
                    p(&[(0, 1)], 0),
                    p(&[(0, 1)], 1),
                    p(&[], 1),
                ],
            ),
            (
                "x̂",
                vec![p(&[], 0), p(&[(0, 0)], 0), p(&[(0, 0)], 1), p(&[], 1)],
            ),
            (
                "x̅",
                vec![p(&[], 0), p(&[(0, 0)], 0), p(&[(0, 0)], 1), p(&[], 1)],
            ),
            (
                "█(a@b)",
                vec![
                    p(&[], 0),
                    p(&[(0, 0)], 0),
                    p(&[(0, 0)], 1),
                    p(&[(0, 1)], 0),
                    p(&[(0, 1)], 1),
                    p(&[], 1),
                ],
            ),
        ];
        for (src, want) in cases {
            let root = parse(src);
            assert_eq!(walk(&root), want, "{src}: {root:?}");
        }
        // lim_(x→0) puts the limit below the name: base, limit, then the argument.
        let root = parse("lim_(n→∞) a");
        let ps = walk(&root);
        assert!(ps.contains(&p(&[(0, 0), (0, 1)], 0)), "{root:?} {ps:?}");
    }

    #[test]
    fn tab_and_shift_tab_go_from_slot_to_slot() {
        let root = parse("a/b+c");
        // From the zone start, Tab enters the numerator; then the denominator; then after the fraction.
        let a = tab(&root, &zone_end(&root, false), false);
        assert_eq!(a, p(&[(0, 0)], 0));
        let b = tab(&root, &a, false);
        assert_eq!(b, p(&[(0, 1)], 0));
        let c = tab(&root, &b, false);
        assert_eq!(c, p(&[], 1));
        // Nothing follows: the caret stays.
        assert_eq!(tab(&root, &c, false), c);
        // Shift+Tab back into the denominator's end, then the numerator's end.
        let d = tab(&root, &c, true);
        assert_eq!(d, p(&[(0, 1)], 1));
        assert_eq!(tab(&root, &d, true), p(&[(0, 0)], 1));
        assert_eq!(all_slots(&root).len(), 3);
        assert_eq!(slot_index(&root, &[Step { item: 0, slot: 1 }]), Some(2));
    }

    #[test]
    fn typing_and_deleting_characters_and_structures() {
        let mut root = parse("a/b");
        // Type into the denominator after b.
        let at = insert_text(&mut root, &p(&[(0, 1)], 1), "c").unwrap();
        assert_eq!(at, p(&[(0, 1)], 2));
        assert_eq!(root, parse("a/bc"));
        // Into an empty slot: a new run.
        let mut e = vec![Node::Frac {
            num: vec![],
            den: vec![],
            bar: true,
            pr: Pr::default(),
        }];
        let at = insert_text(&mut e, &p(&[(0, 0)], 0), "x").unwrap();
        assert_eq!(at, p(&[(0, 0)], 1));
        assert_eq!(
            slot_at(&e, &[Step { item: 0, slot: 0 }]).unwrap(),
            &[r("x")]
        );
        // Backspace removes characters; at the slot start it selects the fraction, then deletes it.
        let d = delete(&mut root, &p(&[(0, 1)], 2), true).unwrap();
        assert_eq!(d, Deleted::Char(p(&[(0, 1)], 1)));
        assert_eq!(root, parse("a/b"));
        let d = delete(&mut root, &p(&[(0, 1)], 0), true).unwrap();
        assert_eq!(d, Deleted::Select(p(&[], 1)));
        assert_eq!(structure_before(&root, &p(&[], 1)), Some(0));
        assert_eq!(delete_selected(&mut root, &p(&[], 1)), Some(p(&[], 0)));
        assert_eq!(root, Vec::<Node>::new());
        // Delete forward before a structure selects it too; at the zone edges nothing happens inside.
        let mut root = parse("x^2+1");
        assert_eq!(
            delete(&mut root, &p(&[], 0), false),
            Some(Deleted::Select(p(&[], 1)))
        );
        assert_eq!(delete(&mut root, &p(&[], 0), true), Some(Deleted::Outside));
        assert_eq!(delete(&mut root, &p(&[], 3), false), Some(Deleted::Outside));
        assert_eq!(
            delete(&mut root, &p(&[], 3), true),
            Some(Deleted::Char(p(&[], 2)))
        );
        assert_eq!(root, parse("x^2+"));
    }

    // source: UnicodeMath build-up (UTN #28): `x^2`, `a/b`, `√`, `∑`, brackets, control words.
    #[test]
    fn autobuild_common_cases() {
        let cases: &[(&str, &str, &str)] = &[
            ("x^2", "x^2", ""),
            ("a/b", "a/b", ""),
            ("y=x^2", "y=x^2", ""),
            ("(a+b)", "(a+b)", ""),
            ("\\alpha", "α", ""),
            ("2\\pi", "2π", ""),
        ];
        for &(typed, want, _) in cases {
            let mut root = vec![r(typed)];
            let n = typed.chars().count();
            let at = autobuild(&mut root, &p(&[], n)).unwrap();
            assert_eq!(root, parse(want), "{typed}");
            assert_eq!(at, p(&[], slot_len(&root)), "{typed}");
        }
        // √ and ∑ build with an empty operand that takes the caret.
        let mut root = vec![r("\\sqrt")];
        let at = autobuild(&mut root, &p(&[], 5)).unwrap();
        assert_eq!(
            root,
            vec![Node::Rad {
                deg: None,
                body: vec![],
                pr: Pr::default(),
            }]
        );
        assert_eq!(at, p(&[(0, 0)], 0));
        let mut root = vec![r("1+\\sum")];
        let at = autobuild(&mut root, &p(&[], 6)).unwrap();
        assert!(matches!(&root[1], Node::Nary { chr: '∑', body, .. } if body.is_empty()));
        assert_eq!(at, p(&[(1, 0)], 0));
        let mut root = vec![r("\\int")];
        autobuild(&mut root, &p(&[], 4)).unwrap();
        assert!(matches!(&root[0], Node::Nary { chr: '∫', .. }));
        // Plain text does not build; text after the caret stays.
        let mut root = vec![r("a+b")];
        assert_eq!(autobuild(&mut root, &p(&[], 3)), None);
        let mut root = vec![r("x^2+1")];
        let at = autobuild(&mut root, &p(&[], 3)).unwrap();
        assert_eq!(root, parse("x^2+1"));
        assert_eq!(at, p(&[], 1));
        // Scripts after a structure apply to it.
        let mut root = parse("a/b");
        root.push(r("^2"));
        let at = autobuild(&mut root, &p(&[], 3)).unwrap();
        assert_eq!(root, parse("(a/b)^2"));
        assert_eq!(at, p(&[], 1));
        // Inside a slot.
        let mut root = parse("a/b");
        insert_text(&mut root, &p(&[(0, 1)], 1), "^2").unwrap();
        let at = autobuild(&mut root, &p(&[(0, 1)], 3)).unwrap();
        assert_eq!(root, parse("a/b^2"));
        assert_eq!(at, p(&[(0, 1)], 1));
    }

    #[test]
    fn clamp_and_normalize() {
        let root = parse("a/b");
        assert_eq!(clamp(&root, &p(&[(0, 1)], 9)), p(&[(0, 1)], 1));
        assert_eq!(clamp(&root, &p(&[(0, 5)], 0)), p(&[], 0));
        assert_eq!(clamp(&root, &p(&[(3, 0)], 0)), p(&[], 1));
        let mut v = vec![
            r("a"),
            r(""),
            r("b"),
            Node::Rad {
                deg: Some(vec![]),
                body: vec![r("x")],
                pr: Pr::default(),
            },
        ];
        normalize(&mut v);
        assert_eq!(v, parse("ab√x"));
    }

    proptest::proptest! {
        // Any sequence of motions and edits keeps the caret valid, and the edited tree, written as OMML
        // and read back, has the same slots (what layout geometry and caret paths rely on).
        #[test]
        fn edits_keep_positions_valid(
            src in proptest::sample::select(vec!["a/b+c", "x_i^2", "√(n&x)", "∑_k^n k", "■(a&b@c&d)", "sin x", "(a+b)", "lim_(n→∞) a_n"]),
            ops in proptest::collection::vec(0u8..9, 0..40),
        ) {
            let mut root = parse(src);
            let mut pos = zone_end(&root, false);
            for op in ops {
                match op {
                    0 => if let Some(n) = right(&root, &pos) { pos = n },
                    1 => if let Some(n) = left(&root, &pos) { pos = n },
                    2 => pos = tab(&root, &pos, false),
                    3 => pos = tab(&root, &pos, true),
                    4 => pos = insert_text(&mut root, &pos, "y").unwrap(),
                    5 => pos = insert_text(&mut root, &pos, "^").unwrap(),
                    6 => if let Some(n) = autobuild(&mut root, &pos) { pos = n },
                    7 => match delete(&mut root, &pos, true).unwrap() {
                        Deleted::Char(n) => pos = n,
                        Deleted::Select(n) => pos = delete_selected(&mut root, &n).unwrap(),
                        Deleted::Outside => {}
                    },
                    _ => match delete(&mut root, &pos, false).unwrap() {
                        Deleted::Char(n) => pos = n,
                        Deleted::Select(n) => pos = delete_selected(&mut root, &n).unwrap(),
                        Deleted::Outside => {}
                    },
                }
                proptest::prop_assert!(is_valid(&root, &pos), "{:?} {:?}", root, pos);
            }
            normalize(&mut root);
            let back = crate::from_omml(&crate::to_omml(&root).replacen("<m:oMath>", &format!(r#"<m:oMath xmlns:m="{}">"#, crate::omml::NS), 1)).unwrap();
            proptest::prop_assert_eq!(all_slots(&back), all_slots(&root));
        }
    }
}
