//! Compare two documents (prompt §7.1 "So sánh"): the revised document with its differences from
//! the original as tracked changes (ECMA-376 Part 1, §17.13.5) — inserted and deleted paragraphs
//! (runs in §17.13.5.18 ins / §17.13.5.14 del, the paragraph mark in §17.13.5.16 / §17.13.5.15),
//! words changed inside a paragraph, formatting changes (§17.13.5.31 rPrChange, §17.13.5.29
//! pPrChange) and tables (cell by cell when the grid is the same, else rows deleted and inserted,
//! §17.13.5.19 trPr ins / §17.13.5.14 del).
//!
//! Accepting every revision of the result gives the revised document, rejecting them the original.

use bo_model::{
    Block, Document, ParaPropsChange, Paragraph, RevMark, RevisionInfo, Row, Run, RunContent,
    RunProps, RunPropsChange, Table,
};

/// Revision ids and attribution of the comparison.
struct Rev {
    info: RevisionInfo,
}

impl Rev {
    fn next(&mut self) -> RevisionInfo {
        self.info.id += 1;
        self.info.clone()
    }

    fn mark(&mut self) -> RevMark {
        RevMark {
            info: self.next(),
            moved: false,
        }
    }
}

/// Longest common subsequence of `a` and `b` by `eq`: matched index pairs in order.
fn lcs<T>(a: &[T], b: &[T], eq: impl Fn(&T, &T) -> bool) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    // Common prefix and suffix first (documents differ in places).
    let mut pre = 0;
    while pre < n && pre < m && eq(&a[pre], &b[pre]) {
        pre += 1;
    }
    let mut suf = 0;
    while suf < n - pre && suf < m - pre && eq(&a[n - 1 - suf], &b[m - 1 - suf]) {
        suf += 1;
    }
    let (ai, bi) = (&a[pre..n - suf], &b[pre..m - suf]);
    let (p, q) = (ai.len(), bi.len());
    let mut out: Vec<(usize, usize)> = (0..pre).map(|i| (i, i)).collect();
    if p > 0 && q > 0 && p.saturating_mul(q) <= 4_000_000 {
        let mut t = vec![0u32; (p + 1) * (q + 1)];
        for i in (0..p).rev() {
            for j in (0..q).rev() {
                t[i * (q + 1) + j] = if eq(&ai[i], &bi[j]) {
                    t[(i + 1) * (q + 1) + j + 1] + 1
                } else {
                    t[(i + 1) * (q + 1) + j].max(t[i * (q + 1) + j + 1])
                };
            }
        }
        let (mut i, mut j) = (0, 0);
        while i < p && j < q {
            if eq(&ai[i], &bi[j]) {
                out.push((pre + i, pre + j));
                i += 1;
                j += 1;
            } else if t[(i + 1) * (q + 1) + j] >= t[i * (q + 1) + j + 1] {
                i += 1;
            } else {
                j += 1;
            }
        }
    }
    out.extend((0..suf).map(|k| (n - suf + k, m - suf + k)));
    out
}

/// One unit of a paragraph's content: a character of text or another content item, with its run
/// properties.
#[derive(Clone)]
struct Atom {
    item: RunContent,
    props: RunProps,
}

fn atoms(p: &Paragraph) -> Vec<Atom> {
    let mut out = Vec::new();
    for r in &p.runs {
        for c in &r.content {
            match c {
                RunContent::Text(t) => out.extend(t.chars().map(|ch| Atom {
                    item: RunContent::Text(ch.to_string()),
                    props: r.props.clone(),
                })),
                other => out.push(Atom {
                    item: other.clone(),
                    props: r.props.clone(),
                }),
            }
        }
    }
    out
}

/// Words, spaces and single other characters / items.
fn tokens(a: &[Atom]) -> Vec<(usize, usize, String)> {
    let class = |x: &Atom| match &x.item {
        RunContent::Text(t) => {
            let ch = t.chars().next().unwrap_or(' ');
            if ch.is_alphanumeric() {
                1
            } else if ch.is_whitespace() {
                2
            } else {
                3
            }
        }
        _ => 4,
    };
    let key = |x: &Atom| match &x.item {
        RunContent::Text(t) => t.clone(),
        other => format!("\u{1}{other:?}"),
    };
    let mut out = Vec::new();
    let mut i = 0;
    while i < a.len() {
        let c = class(&a[i]);
        let mut j = i + 1;
        if c == 1 || c == 2 {
            while j < a.len() && class(&a[j]) == c {
                j += 1;
            }
        }
        out.push((i, j, a[i..j].iter().map(key).collect()));
        i = j;
    }
    out
}

/// Runs of `atoms` grouped by equal properties.
fn runs_of(atoms: &[Atom]) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    for a in atoms {
        let same = out.last().is_some_and(|r| r.props == a.props);
        if !same {
            out.push(Run {
                props: a.props.clone(),
                content: Vec::new(),
            });
        }
        let Some(r) = out.last_mut() else { continue };
        match (&a.item, r.content.last_mut()) {
            (RunContent::Text(t), Some(RunContent::Text(prev))) => prev.push_str(t),
            (item, _) => r.content.push(item.clone()),
        }
    }
    out
}

fn with_mark(atoms: &[Atom], f: impl Fn(&mut RunProps)) -> Vec<Atom> {
    atoms
        .iter()
        .map(|a| {
            let mut a = a.clone();
            f(&mut a.props);
            a
        })
        .collect()
}

/// Run properties without revision marks (what formatting compares).
fn plain(p: &RunProps) -> RunProps {
    let mut p = p.clone();
    p.ins = None;
    p.del = None;
    p.change = None;
    p
}

/// A paragraph of the old document, deleted.
fn deleted_paragraph(p: &Paragraph, rev: &mut Rev) -> Paragraph {
    let mark = rev.mark();
    let a = with_mark(&atoms(p), |r| r.del = Some(mark.clone()));
    let mut props = p.props.clone();
    props.mark_del = Some(rev.mark());
    Paragraph {
        props,
        runs: runs_of(&a),
        src: None,
    }
}

/// A paragraph of the new document, inserted.
fn inserted_paragraph(p: &Paragraph, rev: &mut Rev) -> Paragraph {
    let mark = rev.mark();
    let a = with_mark(&atoms(p), |r| r.ins = Some(mark.clone()));
    let mut props = p.props.clone();
    props.mark_ins = Some(rev.mark());
    Paragraph {
        props,
        runs: runs_of(&a),
        src: None,
    }
}

/// `new` with its differences from `old`: words deleted / inserted, formatting changes.
fn paragraph_diff(old: &Paragraph, new: &Paragraph, rev: &mut Rev) -> Paragraph {
    let (oa, na) = (atoms(old), atoms(new));
    let (ot, nt) = (tokens(&oa), tokens(&na));
    let all = lcs(&ot, &nt, |x, y| x.2 == y.2);
    // A space alone between changed words is part of the change (no "a␣b" → "c␣d" stripes).
    let pairs: Vec<(usize, usize)> = all
        .iter()
        .enumerate()
        .filter(|(k, (pi, pj))| {
            !ot[*pi].2.trim().is_empty()
                || (*k > 0 && all[k - 1] == (pi.wrapping_sub(1), pj.wrapping_sub(1)))
                || all.get(k + 1) == Some(&(pi + 1, pj + 1))
        })
        .map(|(_, p)| *p)
        .collect();
    let mut out: Vec<Atom> = Vec::new();
    let (mut i, mut j) = (0, 0);
    let mut changed = false;
    let push_del = |out: &mut Vec<Atom>, from: usize, to: usize, rev: &mut Rev| {
        if from < to {
            let m = rev.mark();
            out.extend(with_mark(&oa[ot[from].0..ot[to - 1].1], |r| {
                r.del = Some(m.clone());
            }));
        }
    };
    let push_ins = |out: &mut Vec<Atom>, from: usize, to: usize, rev: &mut Rev| {
        if from < to {
            let m = rev.mark();
            out.extend(with_mark(&na[nt[from].0..nt[to - 1].1], |r| {
                r.ins = Some(m.clone());
            }));
        }
    };
    for &(pi, pj) in pairs.iter().chain(std::iter::once(&(ot.len(), nt.len()))) {
        changed |= pi > i || pj > j;
        push_del(&mut out, i, pi, rev);
        push_ins(&mut out, j, pj, rev);
        if pi < ot.len() && pj < nt.len() {
            // Equal words: the new formatting, with the old one recorded where it differs.
            let (o, n) = (&oa[ot[pi].0..ot[pi].1], &na[nt[pj].0..nt[pj].1]);
            for (x, y) in o.iter().zip(n) {
                let mut a = y.clone();
                if plain(&x.props) != plain(&y.props) {
                    changed = true;
                    a.props.change = Some(Box::new(RunPropsChange {
                        info: RevisionInfo::default(),
                        old: plain(&x.props),
                    }));
                }
                out.push(a);
            }
        }
        i = pi + 1;
        j = pj + 1;
    }
    let mut props = new.props.clone();
    let mut po = old.props.clone();
    po.change = None;
    po.mark_ins = None;
    po.mark_del = None;
    let mut pn = new.props.clone();
    pn.change = None;
    if po != pn {
        props.change = Some(Box::new(ParaPropsChange {
            info: rev.next(),
            old: po,
        }));
        changed = true;
    }
    if !changed {
        return new.clone();
    }
    // One revision per formatting change run (the ids of consecutive changed atoms are shared).
    let mut runs = runs_of(&out);
    for r in &mut runs {
        if let Some(c) = r.props.change.as_mut() {
            c.info = rev.next();
        }
    }
    Paragraph {
        props,
        runs,
        src: None,
    }
}

/// Words two paragraphs share, as a share of the longer one (0..1).
fn similarity(a: &Paragraph, b: &Paragraph) -> f64 {
    let (aa, ba) = (atoms(a), atoms(b));
    let (at, bt) = (tokens(&aa), tokens(&ba));
    let words = |t: &[(usize, usize, String)]| -> Vec<String> {
        t.iter()
            .filter(|x| !x.2.trim().is_empty())
            .map(|x| x.2.clone())
            .collect()
    };
    let (aw, bw) = (words(&at), words(&bt));
    let n = aw.len().max(bw.len());
    if n == 0 {
        return if aw.is_empty() && bw.is_empty() {
            1.0
        } else {
            0.0
        };
    }
    lcs(&aw, &bw, |x, y| x == y).len() as f64 / n as f64
}

fn block_key(b: &Block) -> String {
    match b {
        Block::Paragraph(p) => format!("p\u{1}{}", p.plain_text()),
        Block::Table(t) => {
            let mut s = String::from("t");
            for r in &t.rows {
                for c in &r.cells {
                    s.push('\u{1}');
                    for b in &c.blocks {
                        s.push_str(&block_key(b));
                    }
                }
                s.push('\u{2}');
            }
            s
        }
        Block::Opaque(r) => format!("o\u{1}{r:?}"),
    }
}

fn row_marked(r: &Row, ins: Option<RevMark>, del: Option<&RevMark>, rev: &mut Rev) -> Row {
    let mut r = r.clone();
    r.props.ins = ins;
    r.props.del = del.cloned();
    for c in &mut r.cells {
        c.blocks = c
            .blocks
            .iter()
            .map(|b| match b {
                Block::Paragraph(p) if del.is_some() => Block::Paragraph(deleted_paragraph(p, rev)),
                Block::Paragraph(p) => Block::Paragraph(inserted_paragraph(p, rev)),
                other => other.clone(),
            })
            .collect();
    }
    r
}

fn same_grid(a: &Table, b: &Table) -> bool {
    a.rows.len() == b.rows.len()
        && a.rows
            .iter()
            .zip(&b.rows)
            .all(|(x, y)| x.cells.len() == y.cells.len())
}

/// `new` with the differences of `old` in its cells, or `None` when the grids differ.
fn table_diff(old: &Table, new: &Table, rev: &mut Rev) -> Option<Table> {
    if !same_grid(old, new) {
        return None;
    }
    let mut t = new.clone();
    t.src = None;
    for (ri, row) in t.rows.iter_mut().enumerate() {
        for (ci, cell) in row.cells.iter_mut().enumerate() {
            cell.blocks = blocks_diff(&old.rows[ri].cells[ci].blocks, &cell.blocks, rev);
        }
    }
    Some(t)
}

fn deleted_block(b: &Block, rev: &mut Rev) -> Block {
    match b {
        Block::Paragraph(p) => Block::Paragraph(deleted_paragraph(p, rev)),
        Block::Table(t) => {
            let mut t = t.clone();
            t.src = None;
            t.rows = t
                .rows
                .iter()
                .map(|r| {
                    let m = rev.mark();
                    row_marked(r, None, Some(&m), rev)
                })
                .collect();
            Block::Table(t)
        }
        Block::Opaque(_) => b.clone(),
    }
}

fn inserted_block(b: &Block, rev: &mut Rev) -> Block {
    match b {
        Block::Paragraph(p) => Block::Paragraph(inserted_paragraph(p, rev)),
        Block::Table(t) => {
            let mut t = t.clone();
            t.src = None;
            t.rows = t
                .rows
                .iter()
                .map(|r| {
                    let m = rev.mark();
                    row_marked(r, Some(m), None, rev)
                })
                .collect();
            Block::Table(t)
        }
        Block::Opaque(_) => b.clone(),
    }
}

/// The blocks of `new` with their differences from `old`.
fn blocks_diff(old: &[Block], new: &[Block], rev: &mut Rev) -> Vec<Block> {
    let (ok, nk): (Vec<String>, Vec<String>) = (
        old.iter().map(block_key).collect(),
        new.iter().map(block_key).collect(),
    );
    let pairs = lcs(&ok, &nk, |a, b| a == b);
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    for &(pi, pj) in pairs.iter().chain(std::iter::once(&(old.len(), new.len()))) {
        // The blocks between two equal ones: pair alike paragraphs (changed words) and tables of the
        // same grid, in order; the rest are deleted / inserted.
        let (mut a, mut b) = (i, j);
        // One paragraph replaced by one: a change inside it, whatever the words.
        let one_for_one = pi - i == 1 && pj - j == 1;
        let (mut dels, mut inss): (Vec<Block>, Vec<Block>) = (Vec::new(), Vec::new());
        while a < pi || b < pj {
            let pair = match (old.get(a).filter(|_| a < pi), new.get(b).filter(|_| b < pj)) {
                (Some(Block::Paragraph(x)), Some(Block::Paragraph(y)))
                    if one_for_one || similarity(x, y) >= 0.4 =>
                {
                    Some(Block::Paragraph(paragraph_diff(x, y, rev)))
                }
                (Some(Block::Table(x)), Some(Block::Table(y))) => {
                    table_diff(x, y, rev).map(Block::Table)
                }
                _ => None,
            };
            if let Some(p) = pair {
                out.append(&mut dels);
                out.append(&mut inss);
                out.push(p);
                a += 1;
                b += 1;
            } else if a < pi && (b >= pj || (pi - a) >= (pj - b)) {
                dels.push(deleted_block(&old[a], rev));
                a += 1;
            } else {
                inss.push(inserted_block(&new[b], rev));
                b += 1;
            }
        }
        // Deleted blocks, then inserted ones.
        out.append(&mut dels);
        out.append(&mut inss);
        if pi < old.len() && pj < new.len() {
            // Equal text: formatting may still differ.
            out.push(match (&old[pi], &new[pj]) {
                (Block::Paragraph(x), Block::Paragraph(y)) => {
                    Block::Paragraph(paragraph_diff(x, y, rev))
                }
                (Block::Table(x), Block::Table(y)) => {
                    table_diff(x, y, rev).map_or_else(|| new[pj].clone(), Block::Table)
                }
                _ => new[pj].clone(),
            });
        }
        i = pi + 1;
        j = pj + 1;
    }
    last_mark(&mut out);
    out
}

/// The last paragraph mark of a story or cell cannot be a revision (nothing follows to merge
/// with). At the end, deleted paragraphs D… then inserted ones I… become: D… but the last one,
/// then one paragraph holding the last deleted and the first inserted text, then the other
/// inserted ones — the final mark is the last one's, unchanged. Only deletions or only insertions
/// give their mark revision to the paragraph before them (§17.13.5.15 / §17.13.5.16).
fn last_mark(blocks: &mut Vec<Block>) {
    let marked = |b: &Block, ins: bool| match b {
        Block::Paragraph(p) => {
            if ins {
                p.props.mark_ins.is_some()
            } else {
                p.props.mark_del.is_some()
            }
        }
        _ => false,
    };
    let n = blocks.len();
    // Trailing inserted paragraphs, then the deleted ones before them.
    let mut i0 = n;
    while i0 > 0 && marked(&blocks[i0 - 1], true) {
        i0 -= 1;
    }
    let mut d0 = i0;
    while d0 > 0 && marked(&blocks[d0 - 1], false) {
        d0 -= 1;
    }
    if i0 == n && d0 == n {
        return;
    }
    if i0 < n && d0 < i0 {
        // Merge the last deleted paragraph into the first inserted one.
        let Block::Paragraph(last_del) = blocks.remove(i0 - 1) else {
            return;
        };
        if let Block::Paragraph(first_ins) = &mut blocks[i0 - 1] {
            let mut runs = last_del.runs;
            runs.append(&mut first_ins.runs);
            first_ins.runs = runs;
        }
        // The final mark is the original one, unchanged.
        if let Some(Block::Paragraph(last)) = blocks.last_mut() {
            last.props.mark_ins = None;
            last.props.mark_del = None;
        }
        return;
    }
    let n = blocks.len();
    let Some(Block::Paragraph(last)) = blocks.last_mut() else {
        return;
    };
    let (ins, del) = (last.props.mark_ins.take(), last.props.mark_del.take());
    if ins.is_none() && del.is_none() {
        return;
    }
    let mut k = n - 1;
    while k > 0
        && ((ins.is_some() && marked(&blocks[k - 1], true))
            || (del.is_some() && marked(&blocks[k - 1], false)))
    {
        k -= 1;
    }
    if k > 0 {
        if let Block::Paragraph(p) = &mut blocks[k - 1] {
            if p.props.mark_ins.is_none() && p.props.mark_del.is_none() {
                p.props.mark_ins = ins;
                p.props.mark_del = del;
                p.src = None;
            }
        }
    }
}

/// The revised document `new` with its differences from `old` as tracked changes by `author` at
/// `date`; revision ids start after `first_id`.
#[must_use]
pub fn compare(
    old: &Document,
    new: &Document,
    author: &str,
    date: Option<String>,
    first_id: i32,
) -> Document {
    let mut rev = Rev {
        info: RevisionInfo {
            id: first_id,
            author: author.to_owned(),
            date,
        },
    };
    let mut out = new.clone();
    let n = old.sections.len().min(new.sections.len());
    for si in 0..n {
        out.sections[si].blocks =
            blocks_diff(&old.sections[si].blocks, &new.sections[si].blocks, &mut rev);
    }
    // Sections only in one document: all inserted (the old ones' text is appended as deleted).
    for si in n..new.sections.len() {
        out.sections[si].blocks = new.sections[si]
            .blocks
            .iter()
            .map(|b| inserted_block(b, &mut rev))
            .collect();
    }
    if let Some(last) = out.sections.last_mut() {
        for s in old.sections.iter().skip(n) {
            for b in &s.blocks {
                last.blocks.push(deleted_block(b, &mut rev));
            }
        }
    }
    out
}

/// How one revised version treats a block of the original.
#[derive(Clone)]
enum Fate<'a> {
    Same,
    Changed(&'a Paragraph),
    Gone,
}

/// The blocks of `rev` matched to those of `base` (equal, or alike paragraphs within a gap): the
/// fate of each base block and the blocks inserted before each one (index `base.len()` = at the
/// end).
fn fates<'a>(base: &[Block], rev: &'a [Block]) -> (Vec<Fate<'a>>, Vec<Vec<&'a Block>>) {
    let (bk, rk): (Vec<String>, Vec<String>) = (
        base.iter().map(block_key).collect(),
        rev.iter().map(block_key).collect(),
    );
    let pairs = lcs(&bk, &rk, |a, b| a == b);
    let mut fate = vec![Fate::Gone; base.len()];
    let mut ins: Vec<Vec<&Block>> = vec![Vec::new(); base.len() + 1];
    let (mut i, mut j) = (0, 0);
    for &(pi, pj) in pairs
        .iter()
        .chain(std::iter::once(&(base.len(), rev.len())))
    {
        let (mut a, mut b) = (i, j);
        let one_for_one = pi - i == 1 && pj - j == 1;
        while a < pi || b < pj {
            match (
                base.get(a).filter(|_| a < pi),
                rev.get(b).filter(|_| b < pj),
            ) {
                (Some(Block::Paragraph(x)), Some(Block::Paragraph(y)))
                    if one_for_one || similarity(x, y) >= 0.4 =>
                {
                    fate[a] = Fate::Changed(y);
                    a += 1;
                    b += 1;
                }
                _ if a < pi && (b >= pj || (pi - a) >= (pj - b)) => {
                    fate[a] = Fate::Gone;
                    a += 1;
                }
                _ => {
                    ins[a].push(&rev[b]);
                    b += 1;
                }
            }
        }
        if pi < base.len() {
            fate[pi] = match (&base[pi], &rev[pj]) {
                (Block::Paragraph(x), Block::Paragraph(y))
                    if plain_paragraph(x) != plain_paragraph(y) =>
                {
                    Fate::Changed(y)
                }
                _ => Fate::Same,
            };
        }
        i = pi + 1;
        j = pj + 1;
    }
    (fate, ins)
}

/// The revised paragraph of a changed block.
fn changed<'a>(f: &Fate<'a>) -> Option<&'a Paragraph> {
    match f {
        Fate::Changed(q) => Some(q),
        _ => None,
    }
}

/// A paragraph's props and runs without revision marks (what "unchanged" compares).
fn plain_paragraph(p: &Paragraph) -> (bo_model::ParaProps, Vec<(String, RunProps)>) {
    let mut props = p.props.clone();
    props.change = None;
    props.mark_ins = None;
    props.mark_del = None;
    (
        props,
        p.runs
            .iter()
            .map(|r| (r.plain_text(), plain(&r.props)))
            .collect(),
    )
}

/// Word edits of `new` against the base tokens `bt`: whether each base token is deleted, and the
/// atoms inserted before each one (index `bt.len()` = at the end).
fn token_edits(bt: &[(usize, usize, String)], new: &[Atom]) -> (Vec<bool>, Vec<Vec<Atom>>) {
    let nt = tokens(new);
    let all = lcs(bt, &nt, |x, y| x.2 == y.2);
    let pairs: Vec<(usize, usize)> = all
        .iter()
        .enumerate()
        .filter(|(k, (pi, pj))| {
            !bt[*pi].2.trim().is_empty()
                || (*k > 0 && all[k - 1] == (pi.wrapping_sub(1), pj.wrapping_sub(1)))
                || all.get(k + 1) == Some(&(pi + 1, pj + 1))
        })
        .map(|(_, p)| *p)
        .collect();
    let mut del = vec![true; bt.len()];
    let mut ins: Vec<Vec<Atom>> = vec![Vec::new(); bt.len() + 1];
    let (mut i, mut j) = (0, 0);
    for &(pi, pj) in pairs.iter().chain(std::iter::once(&(bt.len(), nt.len()))) {
        if j < pj {
            ins[i].extend(new[nt[j].0..nt[pj - 1].1].iter().cloned());
        }
        if pi < bt.len() {
            del[pi] = false;
        }
        i = pi + 1;
        j = pj + 1;
    }
    (del, ins)
}

/// The base paragraph with the word edits of two reviewers: both deletions; the insertions of
/// the first, then of the second unless the same.
fn merge_paragraph(
    base: &Paragraph,
    a: Option<&Paragraph>,
    b: Option<&Paragraph>,
    ra: &mut Rev,
    rb: &mut Rev,
) -> Paragraph {
    let ba = atoms(base);
    let bt = tokens(&ba);
    let ea = a.map(|p| token_edits(&bt, &atoms(p)));
    let eb = b.map(|p| token_edits(&bt, &atoms(p)));
    let mut out: Vec<Atom> = Vec::new();
    let put_ins = |out: &mut Vec<Atom>, atoms: &[Atom], rev: &mut Rev| {
        if !atoms.is_empty() {
            let m = rev.mark();
            out.extend(with_mark(atoms, |r| r.ins = Some(m.clone())));
        }
    };
    for k in 0..=bt.len() {
        let ia = ea.as_ref().map_or(&[][..], |e| e.1[k].as_slice());
        let ib = eb.as_ref().map_or(&[][..], |e| e.1[k].as_slice());
        put_ins(&mut out, ia, ra);
        let same = ia.len() == ib.len() && ia.iter().zip(ib).all(|(x, y)| x.item == y.item);
        if !same {
            put_ins(&mut out, ib, rb);
        }
        if k < bt.len() {
            let span = &ba[bt[k].0..bt[k].1];
            let da = ea.as_ref().is_some_and(|e| e.0[k]);
            let db = eb.as_ref().is_some_and(|e| e.0[k]);
            if da || db {
                let rev = if da { &mut *ra } else { &mut *rb };
                let m = rev.mark();
                out.extend(with_mark(span, |r| r.del = Some(m.clone())));
            } else {
                out.extend(span.iter().cloned());
            }
        }
    }
    Paragraph {
        props: a
            .or(b)
            .map_or_else(|| base.props.clone(), |p| p.props.clone()),
        runs: runs_of(&out),
        src: None,
    }
}

/// Combine two revised versions of `base` (prompt §7.1 "Kết hợp"): `base` with the changes of
/// `a` (by `author_a`) and of `b` (by `author_b`) as tracked changes. Paragraphs changed by both
/// merge their word edits; another block changed by both keeps the first reviewer's version.
#[must_use]
pub fn combine(
    base: &Document,
    (a, author_a): (&Document, &str),
    (b, author_b): (&Document, &str),
    date: Option<&str>,
    first_id: i32,
) -> Document {
    let rev = |author: &str, id: i32| Rev {
        info: RevisionInfo {
            id,
            author: author.to_owned(),
            date: date.map(str::to_owned),
        },
    };
    let mut ra = rev(author_a, first_id);
    let mut out = base.clone();
    for (si, sec) in out.sections.iter_mut().enumerate() {
        let empty = Vec::new();
        let ab = a.sections.get(si).map_or(&empty, |s| &s.blocks);
        let bb = b.sections.get(si).map_or(&empty, |s| &s.blocks);
        let (fa, ia) = fates(&sec.blocks, ab);
        let (fb, ib) = fates(&sec.blocks, bb);
        let mut blocks = Vec::new();
        for k in 0..=sec.blocks.len() {
            for x in &ia[k] {
                blocks.push(inserted_block(x, &mut ra));
            }
            // The second reviewer's insertions, unless the first made the same.
            let mut rb = rev(author_b, ra.info.id);
            for x in &ib[k] {
                if !ia[k].iter().any(|y| block_key(y) == block_key(x)) {
                    blocks.push(inserted_block(x, &mut rb));
                }
            }
            ra.info.id = rb.info.id;
            let Some(orig) = sec.blocks.get(k) else {
                break;
            };
            let mut rb = rev(author_b, ra.info.id);
            let block = match (orig, &fa[k], &fb[k]) {
                (_, Fate::Same, Fate::Same) => orig.clone(),
                (_, Fate::Gone, _) => deleted_block(orig, &mut ra),
                (_, _, Fate::Gone) => deleted_block(orig, &mut rb),
                (Block::Paragraph(p), x, y) => {
                    Block::Paragraph(merge_paragraph(p, changed(x), changed(y), &mut ra, &mut rb))
                }
                _ => orig.clone(),
            };
            ra.info.id = rb.info.id;
            blocks.push(block);
        }
        last_mark(&mut blocks);
        sec.blocks = blocks;
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::revisions::resolve_blocks;
    use bo_model::{ParaProps, Section};

    fn para(runs: &[(&str, bool)]) -> Block {
        Block::Paragraph(Paragraph::new(
            ParaProps::default(),
            runs.iter()
                .map(|(t, b)| {
                    Run::text(
                        *t,
                        RunProps {
                            bold: b.then_some(true),
                            ..RunProps::default()
                        },
                    )
                })
                .collect(),
        ))
    }

    fn doc(blocks: Vec<Block>) -> Document {
        Document {
            sections: vec![Section {
                blocks,
                ..Section::default()
            }],
            ..Document::default()
        }
    }

    fn text(blocks: &[Block]) -> Vec<String> {
        blocks
            .iter()
            .map(|b| match b {
                Block::Paragraph(p) => p.plain_text(),
                Block::Table(t) => format!(
                    "[{}]",
                    t.rows
                        .iter()
                        .map(|r| r
                            .cells
                            .iter()
                            .map(|c| text(&c.blocks).join("/"))
                            .collect::<Vec<_>>()
                            .join("|"))
                        .collect::<Vec<_>>()
                        .join(";")
                ),
                Block::Opaque(_) => "?".into(),
            })
            .collect()
    }

    /// Accepting everything gives the new text, rejecting everything the old.
    fn check(old: &[Block], new: &[Block]) -> Vec<Block> {
        let out = compare(&doc(old.to_vec()), &doc(new.to_vec()), "BlueDoc", None, 0);
        let blocks = out.sections[0].blocks.clone();
        let acc = resolve_blocks(&blocks, &|_| true, true);
        let rej = resolve_blocks(&blocks, &|_| true, false);
        assert_eq!(text(&acc), text(new), "accept");
        assert_eq!(text(&rej), text(old), "reject");
        blocks
    }

    fn marks(b: &Block) -> Vec<(String, &'static str)> {
        let Block::Paragraph(p) = b else {
            return Vec::new();
        };
        p.runs
            .iter()
            .map(|r| {
                let k = if r.props.ins.is_some() {
                    "ins"
                } else if r.props.del.is_some() {
                    "del"
                } else if r.props.change.is_some() {
                    "fmt"
                } else {
                    ""
                };
                (r.plain_text(), k)
            })
            .collect()
    }

    // source: test pairs of documents; ECMA-376 Part 1, §17.13.5 — the result's revisions turn the
    // original into the revised document and back.
    #[test]
    fn a_changed_word_is_deleted_and_inserted() {
        let out = check(
            &[para(&[("Hợp đồng ký ngày 5 tháng 3.", false)])],
            &[para(&[("Hợp đồng ký ngày 7 tháng 3.", false)])],
        );
        assert_eq!(
            marks(&out[0]),
            vec![
                ("Hợp đồng ký ngày ".into(), ""),
                ("5".into(), "del"),
                ("7".into(), "ins"),
                (" tháng 3.".into(), "")
            ]
        );
    }

    #[test]
    fn paragraphs_inserted_deleted_and_kept() {
        let out = check(
            &[
                para(&[("Một", false)]),
                para(&[("Hai", false)]),
                para(&[("Ba", false)]),
            ],
            &[
                para(&[("Một", false)]),
                para(&[("Ba", false)]),
                para(&[("Bốn mới", false)]),
            ],
        );
        assert_eq!(out.len(), 4);
        assert_eq!(marks(&out[1]), vec![("Hai".into(), "del")]);
        assert!(matches!(&out[1], Block::Paragraph(p) if p.props.mark_del.is_some()));
        assert_eq!(marks(&out[3]), vec![("Bốn mới".into(), "ins")]);
        // The last paragraph mark stays; the one before it is the inserted mark.
        assert!(matches!(&out[3], Block::Paragraph(p) if p.props.mark_ins.is_none()));
        assert!(matches!(&out[2], Block::Paragraph(p) if p.props.mark_ins.is_some()));
    }

    #[test]
    fn formatting_changes_are_recorded() {
        let out = check(
            &[para(&[("Chữ thường và đậm", false)])],
            &[para(&[("Chữ thường và ", false), ("đậm", true)])],
        );
        assert_eq!(
            marks(&out[0]),
            vec![("Chữ thường và ".into(), ""), ("đậm".into(), "fmt")]
        );
        let Block::Paragraph(p) = &out[0] else {
            panic!()
        };
        assert_eq!(p.runs[1].props.change.as_ref().unwrap().old.bold, None);
        // Nothing changed: the revised paragraph as it is.
        let same = check(&[para(&[("Giống", false)])], &[para(&[("Giống", false)])]);
        assert_eq!(marks(&same[0]), vec![("Giống".into(), "")]);
    }

    fn table(cells: &[&[&str]]) -> Block {
        Block::Table(Table {
            rows: cells
                .iter()
                .map(|r| Row {
                    cells: r
                        .iter()
                        .map(|t| bo_model::Cell {
                            blocks: vec![para(&[(t, false)])],
                            ..bo_model::Cell::default()
                        })
                        .collect(),
                    ..Row::default()
                })
                .collect(),
            ..Table::default()
        })
    }

    #[test]
    fn tables_compare_cell_by_cell_or_by_rows() {
        // Same grid: the changed cell only.
        let out = check(
            &[table(&[&["A", "1"], &["B", "2"]])],
            &[table(&[&["A", "1"], &["B", "3"]])],
        );
        let Block::Table(t) = &out[0] else { panic!() };
        assert_eq!(
            marks(&t.rows[1].cells[1].blocks[0]),
            vec![("2".into(), "del"), ("3".into(), "ins")]
        );
        assert!(t.rows[0].props.ins.is_none() && t.rows[0].props.del.is_none());
        // A row more: the old table deleted, the new one inserted.
        let out = check(&[table(&[&["A"]])], &[table(&[&["A"], &["B"]])]);
        assert_eq!(out.len(), 2);
        let (Block::Table(a), Block::Table(b)) = (&out[0], &out[1]) else {
            panic!()
        };
        assert!(a.rows.iter().all(|r| r.props.del.is_some()));
        assert!(b.rows.iter().all(|r| r.props.ins.is_some()));
    }

    #[test]
    fn unrelated_paragraphs_are_replaced() {
        // One paragraph for another: a change inside it.
        let out = check(
            &[para(&[("Hoàn toàn khác", false)])],
            &[para(&[("Một câu mới hẳn", false)])],
        );
        assert_eq!(out.len(), 1);
        assert_eq!(
            marks(&out[0]),
            vec![
                ("Hoàn toàn khác".into(), "del"),
                ("Một câu mới hẳn".into(), "ins")
            ]
        );
        // Several unrelated paragraphs: deleted, then inserted.
        let out = check(
            &[
                para(&[("Đầu", false)]),
                para(&[("Hoàn toàn khác", false)]),
                para(&[("Cũng khác hẳn", false)]),
            ],
            &[
                para(&[("Đầu", false)]),
                para(&[("Một câu mới", false)]),
                para(&[("Câu thứ hai mới", false)]),
            ],
        );
        // Đầu, the first deleted one, the last deleted and first inserted sharing the
        // paragraph, the last inserted one.
        assert_eq!(out.len(), 4);
        assert_eq!(
            marks(&out[2]),
            vec![
                ("Cũng khác hẳn".into(), "del"),
                ("Một câu mới".into(), "ins")
            ]
        );
    }

    // source: prompt §7.1 "Kết hợp" — two reviewers edit copies of one original: the combined
    // document holds both reviewers' changes, each by its author; rejecting all gives the original,
    // accepting all gives both edits.
    #[test]
    fn two_reviewers_combine() {
        let base = doc(vec![
            para(&[("Điều 1: giá trị là 100 triệu.", false)]),
            para(&[("Điều 2: thời hạn một năm.", false)]),
            para(&[("Điều 3: bảo hành.", false)]),
        ]);
        // Lan changes the price and removes Điều 3; Minh changes the term and adds Điều 4.
        let lan = doc(vec![
            para(&[("Điều 1: giá trị là 120 triệu.", false)]),
            para(&[("Điều 2: thời hạn một năm.", false)]),
        ]);
        let minh = doc(vec![
            para(&[("Điều 1: giá trị là 100 triệu.", false)]),
            para(&[("Điều 2: thời hạn hai năm.", false)]),
            para(&[("Điều 3: bảo hành.", false)]),
            para(&[("Điều 4: phạt vi phạm.", false)]),
        ]);
        let out = combine(&base, (&lan, "Lan"), (&minh, "Minh"), None, 0);
        let blocks = &out.sections[0].blocks;
        let rej = resolve_blocks(blocks, &|_| true, false);
        assert_eq!(text(&rej), text(&base.sections[0].blocks));
        let acc = resolve_blocks(blocks, &|_| true, true);
        assert_eq!(
            text(&acc),
            vec![
                "Điều 1: giá trị là 120 triệu.",
                "Điều 2: thời hạn hai năm.",
                "Điều 4: phạt vi phạm."
            ]
        );
        let authors: std::collections::BTreeSet<String> = crate::revisions::list(&out)
            .into_iter()
            .map(|r| r.author)
            .collect();
        assert_eq!(authors.into_iter().collect::<Vec<_>>(), vec!["Lan", "Minh"]);
        // Both change the same sentence: both edits are kept.
        let a = doc(vec![para(&[("Giá 100 triệu.", false)])]);
        let b = doc(vec![para(&[("Giá 100 triệu đồng.", false)])]);
        let c = doc(vec![para(&[("Giá 150 triệu.", false)])]);
        let out = combine(&a, (&c, "Lan"), (&b, "Minh"), None, 0);
        let acc = resolve_blocks(&out.sections[0].blocks, &|_| true, true);
        assert_eq!(text(&acc), vec!["Giá 150 triệu đồng."]);
        let rej = resolve_blocks(&out.sections[0].blocks, &|_| true, false);
        assert_eq!(text(&rej), vec!["Giá 100 triệu."]);
    }
}
