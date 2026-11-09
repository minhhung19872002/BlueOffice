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
///
/// At content-control boundaries (§17.5.2 sdt) the content goes past the end of a control whose
/// value is picked (check box, list, date, picture) and inside a control that starts there; text
/// typed into a control showing its placeholder clears the placeholder flag.
pub fn insert(atoms: &mut Vec<Atom>, off: usize, new: Vec<Atom>) {
    let mut i = split_at(atoms, off);
    // Inline controls open before `i` (atom indices of their starts).
    let mut open: Vec<usize> = Vec::new();
    for (k, (_, c)) in atoms[..i].iter().enumerate() {
        match c {
            RunContent::ControlStart(cc) if !cc.block => open.push(k),
            RunContent::ControlEnd(false) => {
                open.pop();
            }
            _ => {}
        }
    }
    let picked = |cc: &bo_model::ContentControl| {
        use bo_model::ControlKind as K;
        matches!(
            cc.kind,
            K::Checkbox { .. } | K::DropDown(_) | K::Date { .. } | K::Picture
        )
    };
    while let (Some((_, RunContent::ControlEnd(false))), Some(&s)) = (atoms.get(i), open.last()) {
        match &atoms[s].1 {
            RunContent::ControlStart(cc) if picked(cc) => {
                open.pop();
                i += 1;
            }
            _ => break,
        }
    }
    // Content typed where an editable range starts goes inside it
    // (ECMA-376 Part 1, §17.13.7.2 permStart).
    while let Some((_, RunContent::Markup(r))) = atoms.get(i) {
        if !is_perm_start(&r.xml) {
            break;
        }
        i += 1;
    }
    while let Some((_, RunContent::ControlStart(cc))) = atoms.get(i) {
        if !cc.block {
            open.push(i);
        }
        i += 1;
    }
    // At the end of a form field (§17.16.17) the content goes past its end character and the
    // bookmark ends there: typing after a field does not extend its result (filling in edits
    // the result itself), and fields of the classic format do not nest.
    let new_field = new.iter().any(|a| matches!(a.1, RunContent::FormField(_)));
    if new_field {
        i = past_marks(atoms, i);
    } else {
        i = past_field_end(atoms, i);
    }
    let typed = new
        .iter()
        .any(|a| matches!(&a.1, RunContent::Text(t) if !t.is_empty()));
    if typed {
        if let Some(&s) = open.last() {
            if let RunContent::ControlStart(cc) = &mut atoms[s].1 {
                cc.placeholder = false;
            }
        }
    }
    atoms.splice(i..i, new);
}

/// Past the form-field end characters (§17.16.18 fldChar `end`) and bookmark ends at atom `i`.
fn past_marks(atoms: &[Atom], mut i: usize) -> usize {
    while let Some((_, c)) = atoms.get(i) {
        let end_mark = match c {
            RunContent::FormEnd => true,
            RunContent::Markup(r) => r.xml.contains("bookmarkEnd"),
            _ => false,
        };
        if !end_mark {
            break;
        }
        i += 1;
    }
    i
}

/// When atom `i` ends a form field, the index past that end and the bookmark ends after it;
/// else `i`. Typing or splitting at a field's end keeps the field whole.
pub fn past_field_end(atoms: &[Atom], i: usize) -> usize {
    if matches!(atoms.get(i), Some((_, RunContent::FormEnd))) {
        past_marks(atoms, i)
    } else {
        i
    }
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
    // A form field (§17.16.17) wholly inside the range goes with its markers; so does the end
    // character of a removed check box / drop-down field.
    let mut gone = removed
        .iter()
        .filter(|a| matches!(a.1, RunContent::FormField(_)))
        .count();
    let mut out: Vec<Option<Atom>> = Vec::with_capacity(kept.len());
    let mut open: Vec<usize> = Vec::new();
    for at in kept {
        match at.1 {
            RunContent::FormField(_) => {
                open.push(out.len());
                out.push(Some(at));
            }
            RunContent::FormEnd => {
                if let Some(s) = open.pop() {
                    out[s] = None;
                } else if gone > 0 {
                    gone -= 1;
                } else {
                    out.push(Some(at));
                }
            }
            _ => out.push(Some(at)),
        }
    }
    let kept: Vec<Atom> = out.into_iter().flatten().collect();
    let mut k = i + kept.len();
    atoms.splice(i..i, kept);
    while gone > 0 {
        match atoms.get(k) {
            Some((_, RunContent::FormEnd)) => {
                atoms.remove(k);
                gone -= 1;
            }
            // Bookmark / comment marks between the object and its end stay.
            Some((_, c)) if atom_len(c) == 0 && !matches!(c, RunContent::FormField(_)) => k += 1,
            _ => break,
        }
    }
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

/// `w:permStart` markup (§17.13.7.2), whatever the namespace prefix.
fn is_perm_start(xml: &str) -> bool {
    let t = xml.trim_start().trim_start_matches('<');
    let name = t
        .split(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .next()
        .unwrap_or_default();
    name.rsplit(':').next() == Some("permStart")
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
    fn insert_at_control_boundaries() {
        use bo_model::{ContentControl, ControlKind};
        let cc = |kind, placeholder| {
            RunContent::ControlStart(Box::new(ContentControl {
                id: None,
                alias: None,
                tag: None,
                kind,
                lock: None,
                placeholder,
                block: false,
                props_xml: None,
                data_binding: None,
            }))
        };
        let d = RunProps::default();
        let check = ControlKind::Checkbox {
            checked: false,
            on: 'x',
            off: 'o',
        };
        let mut a = vec![
            (d.clone(), cc(check, false)),
            (d.clone(), RunContent::Text("o".into())),
            (d.clone(), RunContent::ControlEnd(false)),
            (d.clone(), cc(ControlKind::Text { multi_line: false }, true)),
            (d.clone(), RunContent::ControlEnd(false)),
        ];
        // After the check box: outside it, and inside the empty text control that starts there.
        insert(&mut a, 1, text_atoms("ab", &d));
        assert!(matches!(&a[2].1, RunContent::ControlEnd(false)));
        assert!(matches!(&a[4].1, RunContent::Text(t) if t == "ab"));
        assert!(matches!(&a[3].1, RunContent::ControlStart(c) if !c.placeholder));
        // At the end of a text control's content the text stays inside it.
        insert(&mut a, 3, text_atoms("c", &d));
        assert!(matches!(&a[5].1, RunContent::Text(t) if t == "c"));
        assert!(matches!(&a[6].1, RunContent::ControlEnd(false)));
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
