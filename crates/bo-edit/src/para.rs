//! Paragraph content primitives over the *index text* (see `bo_model::index`).
//!
//! A paragraph is flattened into atoms `(run properties, content item)`; edits
//! split text atoms at byte offsets, then runs are rebuilt by grouping
//! consecutive atoms with equal properties (ECMA-376 Part 1, §17.3.2.25 r).

use bo_model::index::item_text;
use bo_model::{Paragraph, Run, RunContent, RunProps};

/// One content item with its run's properties.
pub type Atom = (RunProps, RunContent);

/// Flatten a paragraph into atoms.
pub fn flatten(p: &Paragraph) -> Vec<Atom> {
    p.runs
        .iter()
        .flat_map(|r| r.content.iter().map(|c| (r.props.clone(), c.clone())))
        .collect()
}

/// Group atoms back into runs; adjacent text items of a run are merged.
pub fn rebuild(atoms: Vec<Atom>) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for (props, item) in atoms {
        if let RunContent::Text(t) = &item {
            if t.is_empty() {
                continue;
            }
        }
        match runs.last_mut() {
            Some(r) if r.props == props => {
                if let (Some(RunContent::Text(prev)), RunContent::Text(t)) =
                    (r.content.last_mut(), &item)
                {
                    prev.push_str(t);
                } else {
                    r.content.push(item);
                }
            }
            _ => runs.push(Run {
                props,
                content: vec![item],
            }),
        }
    }
    runs
}

/// Byte length of an atom in the index text.
pub fn atom_len(item: &RunContent) -> usize {
    match item {
        RunContent::Text(t) => t.len(),
        other => item_text(other).len(),
    }
}

/// Total index-text length.
pub fn len(atoms: &[Atom]) -> usize {
    atoms.iter().map(|a| atom_len(&a.1)).sum()
}

/// Ensure an atom boundary at `off`; returns the index of the first atom starting at or after `off`.
pub fn split_at(atoms: &mut Vec<Atom>, off: usize) -> usize {
    let mut pos = 0;
    for i in 0..atoms.len() {
        let l = atom_len(&atoms[i].1);
        if pos >= off {
            return i;
        }
        if off < pos + l {
            if let RunContent::Text(t) = &atoms[i].1 {
                let k = off - pos;
                if t.is_char_boundary(k) {
                    let right = t[k..].to_owned();
                    let left = t[..k].to_owned();
                    let props = atoms[i].0.clone();
                    atoms[i].1 = RunContent::Text(left);
                    atoms.insert(i + 1, (props, RunContent::Text(right)));
                    return i + 1;
                }
            }
            // Atomic item: the boundary moves to its end.
            return i + 1;
        }
        pos += l;
    }
    atoms.len()
}

/// Properties for text typed at `off`: the run before the caret, else the first run.
pub fn props_at(atoms: &[Atom], off: usize) -> RunProps {
    let mut pos = 0;
    let mut best: Option<&RunProps> = None;
    for (props, item) in atoms {
        let l = atom_len(item);
        if l > 0 && pos < off {
            best = Some(props);
        }
        pos += l;
    }
    best.or_else(|| atoms.iter().find(|a| atom_len(&a.1) > 0).map(|a| &a.0))
        .or_else(|| atoms.first().map(|a| &a.0))
        .cloned()
        .unwrap_or_default()
}

/// Insert atoms at `off`.
pub fn insert(atoms: &mut Vec<Atom>, off: usize, new: Vec<Atom>) {
    let i = split_at(atoms, off);
    atoms.splice(i..i, new);
}

/// Remove the content in `a..b`; zero-width markup (bookmarks, comments) is kept.
pub fn delete(atoms: &mut Vec<Atom>, a: usize, b: usize) -> Vec<Atom> {
    if b <= a {
        return Vec::new();
    }
    let i = split_at(atoms, a);
    let j = split_at(atoms, b);
    let mut removed = Vec::new();
    let mut kept = Vec::new();
    for at in atoms.drain(i..j) {
        if atom_len(&at.1) == 0 {
            kept.push(at);
        } else {
            removed.push(at);
        }
    }
    atoms.splice(i..i, kept);
    removed
}

/// Copy of the content in `a..b`.
pub fn slice(atoms: &[Atom], a: usize, b: usize) -> Vec<Atom> {
    let mut v = atoms.to_vec();
    let i = split_at(&mut v, a);
    let j = split_at(&mut v, b);
    v[i..j]
        .iter()
        .filter(|a| atom_len(&a.1) > 0)
        .cloned()
        .collect()
}

/// Apply `f` to the run properties of content in `a..b`.
pub fn format(atoms: &mut Vec<Atom>, a: usize, b: usize, f: &dyn Fn(&mut RunProps)) {
    let i = split_at(atoms, a);
    let j = split_at(atoms, b);
    for at in &mut atoms[i..j] {
        f(&mut at.0);
    }
}

/// Paragraph with the given atoms and the original properties.
pub fn with_atoms(p: &Paragraph, atoms: Vec<Atom>) -> Paragraph {
    Paragraph {
        props: p.props.clone(),
        runs: rebuild(atoms),
        src: p.src.clone(),
    }
}

/// Atoms for plain text typed with `props` (`\t` becomes a tab, U+2028 / U+000B a line break).
pub fn text_atoms(text: &str, props: &RunProps) -> Vec<Atom> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let flush = |buf: &mut String, out: &mut Vec<Atom>| {
        if !buf.is_empty() {
            out.push((props.clone(), RunContent::Text(std::mem::take(buf))));
        }
    };
    for c in text.chars() {
        let special = match c {
            '\t' => Some(RunContent::Tab),
            '\u{2028}' | '\u{000B}' => Some(RunContent::LineBreak),
            '\u{000C}' => Some(RunContent::PageBreak),
            '\u{2011}' => Some(RunContent::NoBreakHyphen),
            '\r' | '\n' => None,
            _ => {
                buf.push(c);
                continue;
            }
        };
        flush(&mut buf, &mut out);
        if let Some(s) = special {
            out.push((props.clone(), s));
        }
    }
    flush(&mut buf, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn para(parts: &[(&str, bool)]) -> Paragraph {
        Paragraph::new(
            bo_model::ParaProps::default(),
            parts
                .iter()
                .map(|(t, b)| {
                    Run::text(
                        *t,
                        RunProps {
                            bold: b.then_some(true),
                            ..Default::default()
                        },
                    )
                })
                .collect(),
        )
    }

    #[test]
    fn split_insert_delete() {
        let p = para(&[("Xin ", false), ("chào", true)]);
        let mut a = flatten(&p);
        insert(&mut a, 2, text_atoms("ZZ", &RunProps::default()));
        assert_eq!(with_atoms(&p, a.clone()).plain_text(), "XiZZn chào");
        let gone = delete(&mut a, 1, 6);
        assert_eq!(rebuild(gone)[0].plain_text(), "iZZn ");
        let q = with_atoms(&p, a);
        assert_eq!(q.plain_text(), "Xchào");
        assert_eq!(q.runs.len(), 2);
    }

    #[test]
    fn format_range_merges_runs() {
        let p = para(&[("abc", false), ("def", true)]);
        let mut a = flatten(&p);
        format(&mut a, 0, 3, &|r| r.bold = Some(true));
        let q = with_atoms(&p, a);
        assert_eq!(q.runs.len(), 1);
        assert_eq!(q.plain_text(), "abcdef");
        // Multi-byte Vietnamese characters split on char boundaries only.
        let p = para(&[("việt", false)]);
        let mut a = flatten(&p);
        let s = slice(&a, 1, "việ".len());
        assert_eq!(rebuild(s)[0].plain_text(), "iệ");
        format(&mut a, 1, 3, &|r| r.italic = Some(true));
        assert_eq!(with_atoms(&p, a).plain_text(), "việt");
    }

    #[test]
    fn typing_props() {
        let p = para(&[("ab", false), ("cd", true)]);
        let a = flatten(&p);
        assert_eq!(props_at(&a, 2).bold, None);
        assert_eq!(props_at(&a, 3).bold, Some(true));
        assert_eq!(props_at(&a, 0).bold, None);
        assert_eq!(props_at(&[], 0), RunProps::default());
        let t = text_atoms("a\tb\u{2028}c", &RunProps::default());
        assert_eq!(t.len(), 5);
    }
}
