//! Mail-merge rule fields evaluated once per record (ECMA-376 Part 1, §17.16.5):
//! §17.16.5.35 MERGEFIELD, §17.16.5.26 IF, §17.16.5.58 SKIPIF, §17.16.5.39 NEXTIF,
//! §17.16.5.38 NEXT, §17.16.5.36 MERGEREC, §17.16.5.37 MERGESEQ, §17.16.5 ASK (sets a
//! bookmark from a prompt), §17.16.5 FILLIN (the prompt's answer is the result),
//! §17.16.5.57 SET and REF (§17.16.5.51) of a bookmark set by SET / ASK.
//!
//! Fields are read both as simple fields (§17.16.19 fldSimple, [`RunContent::Field`])
//! and as complex fields (§17.16.18 fldChar begin … separate … end, §17.16.23
//! instrText) whose instructions may hold nested fields: a nested field's result
//! is substituted into the instruction before the outer field is evaluated
//! (§17.16.1 "fields may be nested").
//!
//! Numbering (our reading of §17.16.5.36 / §17.16.5.37): MERGEREC is the 1-based
//! position of the record in the recipient list after filtering and sorting;
//! MERGESEQ counts the merged documents produced so far, so records dropped by
//! SKIPIF do not advance it.

use super::{merge_field_name, merge_text, placeholder, MergeData};
use crate::fields::{compare_values, field_kind, mark_of, Mark};
use crate::para::{self, Atom};
use bo_model::field::{format, has_switch, switch_values, words, FieldValue};
use bo_model::{Block, Document, Paragraph, Raw, RunContent, RunProps};
use std::collections::{BTreeMap, HashMap};

/// Answers to ASK / FILLIN prompts: prompt key ([`Prompt::key`]) → one answer used
/// for every record, or one answer per record (by position in the recipient list).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Answers(pub BTreeMap<String, Vec<String>>);

impl Answers {
    /// Answer of prompt `key` for the record at position `pos`.
    pub fn get(&self, key: &str, pos: usize) -> Option<&str> {
        let v = self.0.get(key)?;
        if v.len() == 1 { v.first() } else { v.get(pos) }.map(String::as_str)
    }
}

/// A prompt of an ASK or FILLIN field (what the host asks before merging).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    /// Key of the answer: `ASK:<bookmark>` or `FILLIN:<prompt>`.
    pub key: String,
    /// `ask` or `fillin`.
    pub kind: String,
    /// Bookmark an ASK field sets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Prompt text.
    pub prompt: String,
    /// Default answer (`\d`).
    pub default: String,
    /// Ask once for the whole merge (`\o`), not for every record.
    pub once: bool,
}

/// What a merge pass reads: the data, the recipient order and the prompt answers.
#[derive(Debug, Clone, Copy)]
pub struct MergeRun<'a> {
    /// Data source.
    pub data: &'a MergeData,
    /// Data rows of the recipient list (filtered, sorted, without excluded rows).
    pub order: &'a [usize],
    /// Answers to ASK / FILLIN prompts.
    pub answers: &'a Answers,
}

/// One merged document and the recipients it used.
#[derive(Debug, Clone, PartialEq)]
pub struct MergedDoc {
    /// The document with every merge field replaced by text.
    pub doc: Document,
    /// Positions (in the recipient list) of the records it shows: the first, then
    /// those NEXT / NEXTIF moved to.
    pub records: Vec<usize>,
}

/// Evaluation state of one merged document.
struct Ctx<'a> {
    run: MergeRun<'a>,
    /// Position of the current record in `run.order` (may be past the end).
    pos: usize,
    /// MERGESEQ of the document being built.
    seq: usize,
    /// Bookmark values (SET / ASK, and the template's bookmarks), lower-case names.
    vars: HashMap<String, String>,
    /// SKIPIF met: the document is dropped.
    skip: bool,
    /// Show `«name»` placeholders instead of values (preview without a record).
    placeholders: bool,
}

impl Ctx<'_> {
    fn row(&self) -> Option<usize> {
        self.run.order.get(self.pos).copied()
    }

    fn advance(&mut self) {
        self.pos = (self.pos + 1).min(self.run.order.len());
    }
}

/// A complex field (§17.16.18): begin mark, instruction, separate mark, result, end mark.
#[derive(Debug, Clone)]
struct Complex {
    begin: Atom,
    instr: Vec<Node>,
    sep: Option<Atom>,
    result: Vec<Node>,
    end: Atom,
}

/// Atoms stay inline (they are the common case); complex fields are boxed.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
enum Node {
    Atom(Atom),
    Field(Box<Complex>),
}

struct Open {
    begin: Atom,
    instr: Vec<Node>,
    sep: Option<Atom>,
    result: Vec<Node>,
}

fn push(stack: &mut [Open], top: &mut Vec<Node>, n: Node) {
    match stack.last_mut() {
        Some(o) if o.sep.is_some() => o.result.push(n),
        Some(o) => o.instr.push(n),
        None => top.push(n),
    }
}

/// Field tree of a paragraph's atoms; fields not closed inside the paragraph stay flat.
fn parse(atoms: Vec<Atom>) -> Vec<Node> {
    let mut stack: Vec<Open> = Vec::new();
    let mut top = Vec::new();
    for a in atoms {
        let mark = if a.0.del.is_some() {
            None
        } else {
            mark_of(&a.1)
        };
        match mark {
            Some(Mark::Begin) => stack.push(Open {
                begin: a,
                instr: Vec::new(),
                sep: None,
                result: Vec::new(),
            }),
            Some(Mark::Separate) if stack.last().is_some_and(|o| o.sep.is_none()) => {
                if let Some(o) = stack.last_mut() {
                    o.sep = Some(a);
                }
            }
            Some(Mark::End) if !stack.is_empty() => {
                if let Some(o) = stack.pop() {
                    let f = Node::Field(Box::new(Complex {
                        begin: o.begin,
                        instr: o.instr,
                        sep: o.sep,
                        result: o.result,
                        end: a,
                    }));
                    push(&mut stack, &mut top, f);
                }
            }
            _ => push(&mut stack, &mut top, Node::Atom(a)),
        }
    }
    while let Some(o) = stack.pop() {
        let mut flat = vec![Node::Atom(o.begin)];
        flat.extend(o.instr);
        flat.extend(o.sep.map(Node::Atom));
        flat.extend(o.result);
        for n in flat {
            push(&mut stack, &mut top, n);
        }
    }
    top
}

fn to_atoms(nodes: Vec<Node>, out: &mut Vec<Atom>) {
    for n in nodes {
        match n {
            Node::Atom(a) => out.push(a),
            Node::Field(f) => {
                out.push(f.begin);
                to_atoms(f.instr, out);
                out.extend(f.sep);
                to_atoms(f.result, out);
                out.push(f.end);
            }
        }
    }
}

fn plain(nodes: &[Node]) -> String {
    let mut s = String::new();
    for n in nodes {
        match n {
            Node::Atom((_, c)) => c.push_text(&mut s),
            Node::Field(f) => s.push_str(&plain(&f.result)),
        }
    }
    s
}

/// A nested field's result as it goes into the outer instruction: quoted (with `\"`
/// escapes, §17.16.1) unless it lands inside a quoted string already.
fn quote(v: &str, quoted: bool) -> String {
    let e = v.replace('"', "\\\"");
    if quoted {
        e
    } else {
        format!("\"{e}\"")
    }
}

/// Instruction text of a complex field with nested fields evaluated.
fn instr_string(nodes: &[Node], ctx: &mut Ctx<'_>) -> (String, bool) {
    let mut s = String::new();
    let mut quoted = false;
    let mut nested = false;
    for n in nodes {
        let piece = match n {
            Node::Atom((_, RunContent::Field { instr, cached, .. })) => {
                nested = true;
                let v = eval_instr(instr, ctx).unwrap_or_else(|| cached.clone());
                s.push_str(&quote(&v, quoted));
                continue;
            }
            Node::Atom((_, c)) => match mark_of(c) {
                Some(Mark::Instr(t)) => t,
                _ => match c {
                    RunContent::Text(t) => t.clone(),
                    _ => String::new(),
                },
            },
            Node::Field(f) => {
                nested = true;
                let v = field_text(f, ctx);
                s.push_str(&quote(&v, quoted));
                continue;
            }
        };
        let mut prev = ' ';
        for c in piece.chars() {
            if c == '"' && prev != '\\' {
                quoted = !quoted;
            }
            prev = c;
        }
        s.push_str(&piece);
    }
    (s, nested)
}

/// Result text of a nested complex field.
fn field_text(f: &Complex, ctx: &mut Ctx<'_>) -> String {
    let (instr, _) = instr_string(&f.instr, ctx);
    eval_instr(&instr, ctx).unwrap_or_else(|| plain(&f.result))
}

/// Tokens of a condition (`a op b true false`): quoted strings stay whole, comparison
/// operators split off even when glued (`x>5`). (text, was quoted).
fn tokens(s: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut had = false;
    let mut it = s.chars().peekable();
    let flush = |cur: &mut String, had: &mut bool, out: &mut Vec<(String, bool)>| {
        if *had || !cur.is_empty() {
            out.push((std::mem::take(cur), *had));
        }
        *had = false;
    };
    while let Some(c) = it.next() {
        match c {
            '"' => {
                quoted = !quoted;
                had = true;
            }
            '\\' if quoted && it.peek() == Some(&'"') => {
                cur.push('"');
                it.next();
            }
            c if quoted => cur.push(c),
            c if c.is_whitespace() => flush(&mut cur, &mut had, &mut out),
            '<' | '>' | '=' => {
                flush(&mut cur, &mut had, &mut out);
                let mut op = c.to_string();
                if let Some(&n) = it.peek() {
                    if (c == '<' && (n == '=' || n == '>')) || (c == '>' && n == '=') {
                        op.push(n);
                        it.next();
                    }
                }
                out.push((op, false));
            }
            c => cur.push(c),
        }
    }
    flush(&mut cur, &mut had, &mut out);
    out
}

const OPS: [&str; 6] = ["=", "<>", "<", "<=", ">", ">="];

/// Evaluate `a op b` at the start of `t` (after the field kind); returns the outcome and
/// the number of tokens used.
fn condition(t: &[(String, bool)], ctx: &Ctx<'_>) -> Option<(bool, usize)> {
    let (a, op, b) = (t.first()?, t.get(1)?, t.get(2)?);
    if op.1 || !OPS.contains(&op.0.as_str()) {
        return None;
    }
    // An unquoted operand naming a bookmark reads its value (§17.16.5.26).
    let val = |x: &(String, bool)| {
        if x.1 {
            x.0.clone()
        } else {
            ctx.vars
                .get(&x.0.to_lowercase())
                .cloned()
                .unwrap_or_else(|| x.0.clone())
        }
    };
    Some((compare_values(&val(a), &op.0, &val(b)), 3))
}

/// Tokens of an instruction after its kind, up to the first switch.
fn args(instr: &str) -> Vec<(String, bool)> {
    let t = tokens(instr);
    t.into_iter()
        .skip(1)
        .take_while(|(w, q)| *q || !w.starts_with('\\'))
        .collect()
}

/// Prompt key of an ASK (`ASK:<bookmark>`) or FILLIN (`FILLIN:<prompt>`) instruction.
fn prompt_of(instr: &str) -> Option<Prompt> {
    let kind = field_kind(instr);
    let w = words(instr);
    let default = switch_values(instr, "\\d")
        .first()
        .cloned()
        .unwrap_or_default();
    let once = has_switch(instr, "\\o");
    let free: Vec<String> = w
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(i, x)| {
            !x.starts_with('\\') && !w.get(i - 1).is_some_and(|p| p.eq_ignore_ascii_case("\\d"))
        })
        .map(|(_, x)| x.clone())
        .collect();
    match kind.as_str() {
        "ASK" => {
            let name = free.first()?.clone();
            Some(Prompt {
                key: format!("ASK:{name}"),
                kind: "ask".into(),
                name: Some(name),
                prompt: free.get(1).cloned().unwrap_or_default(),
                default,
                once,
            })
        }
        "FILLIN" => {
            let prompt = free.first().cloned().unwrap_or_default();
            Some(Prompt {
                key: format!("FILLIN:{prompt}"),
                kind: "fillin".into(),
                name: None,
                prompt,
                default,
                once,
            })
        }
        _ => None,
    }
}

/// Result of a merge-related instruction; `None` for other fields (kept as they are).
fn eval_instr(instr: &str, ctx: &mut Ctx<'_>) -> Option<String> {
    let kind = field_kind(instr);
    if ctx.placeholders {
        return (kind == "MERGEFIELD")
            .then(|| merge_field_name(instr).map(|n| placeholder(&n)))
            .flatten();
    }
    let text = |s: String| format(&FieldValue::Text(s), instr, true);
    match kind.as_str() {
        // §17.16.5.35: the value of the column in the current record.
        "MERGEFIELD" => {
            let name = merge_field_name(instr)?;
            let v = ctx
                .row()
                .and_then(|r| ctx.run.data.value(r, &name))
                .unwrap_or("");
            Some(merge_text(instr, v))
        }
        "MERGEREC" => Some(
            ctx.row()
                .map(|_| (ctx.pos + 1).to_string())
                .unwrap_or_default(),
        ),
        "MERGESEQ" => Some(ctx.seq.to_string()),
        // §17.16.5.38: the next record goes into the same document.
        "NEXT" => {
            ctx.advance();
            Some(String::new())
        }
        "NEXTIF" | "SKIPIF" => {
            let a = args(instr);
            if condition(&a, ctx).is_some_and(|(ok, _)| ok) {
                if kind == "NEXTIF" {
                    ctx.advance();
                } else {
                    ctx.skip = true;
                }
            }
            Some(String::new())
        }
        // §17.16.5.26: `IF a op b "true text" "false text"`.
        "IF" => {
            let a = args(instr);
            let (ok, used) = condition(&a, ctx)?;
            let pick = a.get(if ok { used } else { used + 1 });
            Some(text(pick.map(|p| p.0.clone()).unwrap_or_default()))
        }
        "SET" => {
            let a = args(instr);
            let name = a.first()?.0.to_lowercase();
            let v = a.get(1).map(|v| v.0.clone()).unwrap_or_default();
            ctx.vars.insert(name, v);
            Some(String::new())
        }
        "ASK" | "FILLIN" => {
            let p = prompt_of(instr)?;
            let v = ctx
                .run
                .answers
                .get(&p.key, ctx.pos)
                .map_or_else(|| p.default.clone(), str::to_owned);
            if let Some(n) = &p.name {
                ctx.vars.insert(n.to_lowercase(), v);
                Some(String::new())
            } else {
                Some(text(v))
            }
        }
        // REF of a bookmark set during the merge.
        "REF" => {
            let a = args(instr);
            let v = ctx.vars.get(&a.first()?.0.to_lowercase())?.clone();
            Some(text(v))
        }
        _ => None,
    }
}

fn is_comment(c: &RunContent) -> bool {
    matches!(
        c,
        RunContent::CommentStart(_) | RunContent::CommentEnd(_) | RunContent::CommentRef(_)
    )
}

fn separate_mark() -> Atom {
    (
        RunProps::default(),
        RunContent::Opaque(Raw::new("<w:fldChar w:fldCharType=\"separate\"/>", 0)),
    )
}

/// Evaluate `nodes`; `keep` keeps the fields (preview: only their results change),
/// otherwise merge-related fields become plain text and comments are dropped.
fn eval_nodes(
    nodes: Vec<Node>,
    ctx: &mut Ctx<'_>,
    keep: bool,
    out: &mut Vec<Atom>,
    changed: &mut bool,
) {
    for n in nodes {
        match n {
            Node::Atom((
                props,
                RunContent::Field {
                    instr,
                    cached,
                    locked: false,
                },
            )) => match eval_instr(&instr, ctx) {
                Some(v) => {
                    *changed = true;
                    if keep {
                        out.push((
                            props,
                            RunContent::Field {
                                instr,
                                cached: v,
                                locked: false,
                            },
                        ));
                    } else if !v.is_empty() {
                        out.push((props, RunContent::Text(v)));
                    }
                }
                None => out.push((
                    props,
                    RunContent::Field {
                        instr,
                        cached,
                        locked: false,
                    },
                )),
            },
            Node::Atom((_, c)) if !keep && is_comment(&c) => *changed = true,
            Node::Atom(a) => out.push(a),
            Node::Field(f) => {
                let (instr, nested) = instr_string(&f.instr, ctx);
                if let Some(v) = eval_instr(&instr, ctx) {
                    *changed = true;
                    let props = first_props(&f.result).unwrap_or_else(|| f.begin.0.clone());
                    if keep {
                        out.push(f.begin);
                        to_atoms(f.instr, out);
                        out.push(f.sep.unwrap_or_else(separate_mark));
                        if !v.is_empty() {
                            out.push((props, RunContent::Text(v)));
                        }
                        out.push(f.end);
                    } else if !v.is_empty() {
                        out.push((props, RunContent::Text(v)));
                    }
                } else {
                    // Another field (HYPERLINK, PAGE, …): its instruction gets the nested
                    // results and its result region is merged too.
                    let begin_props = f.begin.0.clone();
                    out.push(f.begin);
                    if nested && !keep {
                        *changed = true;
                        let x = instr
                            .replace('&', "&amp;")
                            .replace('<', "&lt;")
                            .replace('>', "&gt;");
                        out.push((
                            begin_props,
                            RunContent::Opaque(Raw::new(
                                format!("<w:instrText xml:space=\"preserve\">{x}</w:instrText>"),
                                0,
                            )),
                        ));
                    } else {
                        to_atoms(f.instr, out);
                    }
                    if let Some(sep) = f.sep {
                        out.push(sep);
                        eval_nodes(f.result, ctx, keep, out, changed);
                    }
                    out.push(f.end);
                }
            }
        }
    }
}

fn first_props(nodes: &[Node]) -> Option<RunProps> {
    nodes.iter().find_map(|n| match n {
        Node::Atom((p, RunContent::Text(_))) => Some(p.clone()),
        Node::Field(f) => first_props(&f.result),
        Node::Atom(_) => None,
    })
}

fn has_fields(p: &Paragraph, keep: bool) -> bool {
    p.runs.iter().flat_map(|r| &r.content).any(|c| {
        matches!(c, RunContent::Field { .. }) || mark_of(c).is_some() || (!keep && is_comment(c))
    })
}

fn eval_paragraph(p: &mut Paragraph, ctx: &mut Ctx<'_>, keep: bool) {
    if !has_fields(p, keep) {
        return;
    }
    let nodes = parse(para::flatten(p));
    let mut out = Vec::new();
    let mut changed = false;
    eval_nodes(nodes, ctx, keep, &mut out, &mut changed);
    if changed {
        p.runs = para::rebuild(out);
        p.src = None;
    }
}

fn eval_blocks(blocks: &mut [Block], ctx: &mut Ctx<'_>, keep: bool) {
    for b in blocks {
        match b {
            Block::Paragraph(p) => eval_paragraph(p, ctx, keep),
            Block::Table(t) => {
                for row in &mut t.rows {
                    for c in &mut row.cells {
                        eval_blocks(&mut c.blocks, ctx, keep);
                    }
                }
            }
            Block::Opaque(_) => {}
        }
    }
}

/// Body in order, then headers and footers with the document's first record (NEXT
/// does not move the record there).
fn eval_doc(doc: &mut Document, ctx: &mut Ctx<'_>, keep: bool) {
    let start = ctx.pos;
    for s in &mut doc.sections {
        eval_blocks(&mut s.blocks, ctx, keep);
    }
    let end = ctx.pos;
    ctx.pos = start;
    for s in &mut doc.sections {
        for hf in [&mut s.headers, &mut s.footers] {
            for b in [&mut hf.default, &mut hf.first, &mut hf.even]
                .into_iter()
                .flatten()
            {
                eval_blocks(b, ctx, keep);
            }
        }
    }
    ctx.pos = end;
}

fn initial_vars(doc: &Document) -> HashMap<String, String> {
    crate::fields::bookmarks(doc)
        .into_iter()
        .map(|b| (b.name.to_lowercase(), b.text.trim().to_owned()))
        .collect()
}

/// Merge `doc` once per output document: each starts at the next unused record; NEXT /
/// NEXTIF move to further records inside it; SKIPIF drops it.
pub fn merge_each(doc: &Document, run: MergeRun<'_>) -> Vec<MergedDoc> {
    let vars = initial_vars(doc);
    let mut out = Vec::new();
    let mut pos = 0;
    let mut seq = 0;
    let n = run.order.len();
    while pos < n {
        let mut copy = doc.clone();
        copy.comments.clear();
        let mut ctx = Ctx {
            run,
            pos,
            seq: seq + 1,
            vars: vars.clone(),
            skip: false,
            placeholders: false,
        };
        eval_doc(&mut copy, &mut ctx, false);
        let last = ctx.pos.min(n - 1);
        if !ctx.skip {
            seq += 1;
            out.push(MergedDoc {
                doc: copy,
                records: (pos..=last).collect(),
            });
        }
        pos = last.max(pos) + 1;
    }
    out
}

/// Copy of `doc` showing the record at position `pos` of the recipient list in its
/// merge fields (fields are kept; for the preview); `None` shows `«name»` placeholders.
pub fn preview_run(doc: &Document, run: MergeRun<'_>, pos: Option<usize>) -> Document {
    let mut d = doc.clone();
    let mut ctx = Ctx {
        run,
        pos: pos.unwrap_or(0),
        seq: pos.unwrap_or(0) + 1,
        vars: initial_vars(doc),
        skip: false,
        placeholders: pos.is_none(),
    };
    eval_doc(&mut d, &mut ctx, true);
    d
}

/// ASK / FILLIN prompts of `doc` in document order (each key once).
pub fn prompts(doc: &Document) -> Vec<Prompt> {
    fn collect(nodes: &[Node], out: &mut Vec<Prompt>) {
        for n in nodes {
            match n {
                Node::Atom((_, RunContent::Field { instr, .. })) => out.extend(prompt_of(instr)),
                Node::Atom(_) => {}
                Node::Field(f) => {
                    collect(&f.instr, out);
                    let mut s = String::new();
                    for x in &f.instr {
                        match x {
                            Node::Atom((_, c)) => {
                                if let Some(Mark::Instr(t)) = mark_of(c) {
                                    s.push_str(&t);
                                }
                            }
                            Node::Field(g) => s.push_str(&quote(&plain(&g.result), false)),
                        }
                    }
                    out.extend(prompt_of(s.trim()));
                    collect(&f.result, out);
                }
            }
        }
    }
    fn blocks(v: &[Block], out: &mut Vec<Prompt>) {
        for b in v {
            match b {
                Block::Paragraph(p) => {
                    if has_fields(p, true) {
                        collect(&parse(para::flatten(p)), out);
                    }
                }
                Block::Table(t) => {
                    for r in &t.rows {
                        for c in &r.cells {
                            blocks(&c.blocks, out);
                        }
                    }
                }
                Block::Opaque(_) => {}
            }
        }
    }
    let mut out = Vec::new();
    for s in &doc.sections {
        blocks(&s.blocks, &mut out);
        for hf in [&s.headers, &s.footers] {
            for b in [&hf.default, &hf.first, &hf.even].into_iter().flatten() {
                blocks(b, &mut out);
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    out.retain(|p| seen.insert(p.key.clone()));
    out
}

/// Atoms of a complex field `kind …` whose instruction holds `parts`: literal
/// instruction text, or a nested `MERGEFIELD name` (§17.16.18, nested fields §17.16.1).
/// The nested fields have no result in the template, so only the outer result shows.
pub fn complex_field(parts: &[RulePart], result: &str, props: &RunProps) -> Vec<Atom> {
    let mark = |x: String| (props.clone(), RunContent::Opaque(Raw::new(x, 0)));
    let instr = |t: &str| {
        mark(format!(
            "<w:instrText xml:space=\"preserve\">{}</w:instrText>",
            t.replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
        ))
    };
    let begin = || mark("<w:fldChar w:fldCharType=\"begin\"/>".into());
    let end = || mark("<w:fldChar w:fldCharType=\"end\"/>".into());
    let mut out = vec![begin()];
    for p in parts {
        match p {
            RulePart::Text(t) => out.push(instr(t)),
            RulePart::Field(name) => {
                let quoted = if name.contains(' ') {
                    format!("\"{name}\"")
                } else {
                    name.clone()
                };
                out.push(begin());
                out.push(instr(&format!(" MERGEFIELD {quoted} ")));
                out.push(end());
            }
        }
    }
    out.push(mark("<w:fldChar w:fldCharType=\"separate\"/>".into()));
    if !result.is_empty() {
        out.push((props.clone(), RunContent::Text(result.to_owned())));
    }
    out.push(end());
    out
}

/// Piece of a rule field's instruction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RulePart {
    /// Literal instruction text.
    Text(String),
    /// A nested `MERGEFIELD` of this column.
    Field(String),
}

/// A merge rule the Rules menu inserts.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum MergeRule {
    /// `IF {MERGEFIELD field} op "value" "then" "else"` (§17.16.5.26).
    If {
        /// Column compared.
        field: String,
        /// `=`, `<>`, `<`, `<=`, `>`, `>=`.
        op: String,
        /// Value compared with.
        value: String,
        /// Text when the comparison holds.
        then: String,
        /// Text otherwise.
        #[serde(default, rename = "else")]
        otherwise: String,
    },
    /// `SKIPIF {MERGEFIELD field} op "value"` (§17.16.5.58).
    SkipIf {
        /// Column compared.
        field: String,
        /// Comparison operator.
        op: String,
        /// Value compared with.
        value: String,
    },
    /// `NEXTIF {MERGEFIELD field} op "value"` (§17.16.5.39).
    NextIf {
        /// Column compared.
        field: String,
        /// Comparison operator.
        op: String,
        /// Value compared with.
        value: String,
    },
    /// `NEXT` (§17.16.5.38).
    Next,
    /// `MERGEREC` (§17.16.5.36).
    MergeRec,
    /// `MERGESEQ` (§17.16.5.37).
    MergeSeq,
    /// `ASK name "prompt" \d "default" [\o]` (§17.16.5 ASK).
    Ask {
        /// Bookmark set to the answer.
        name: String,
        /// Prompt text.
        prompt: String,
        /// Default answer.
        #[serde(default)]
        default: String,
        /// Ask once for the whole merge.
        #[serde(default)]
        once: bool,
    },
    /// `FILLIN "prompt" \d "default" [\o]` (§17.16.5 FILLIN).
    FillIn {
        /// Prompt text.
        prompt: String,
        /// Default answer.
        #[serde(default)]
        default: String,
        /// Ask once for the whole merge.
        #[serde(default)]
        once: bool,
    },
    /// `SET name "value"` (§17.16.5.57).
    Set {
        /// Bookmark set.
        name: String,
        /// Its value.
        value: String,
    },
}

fn q(s: &str) -> String {
    format!("\"{}\"", s.replace('"', "\\\""))
}

fn bookmark_name(s: &str) -> Option<String> {
    let n: String = s
        .trim()
        .chars()
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .collect();
    (!n.is_empty() && !n.contains('"')).then_some(n)
}

impl MergeRule {
    /// Instruction parts and the result shown before merging.
    pub fn parts(&self) -> Option<(Vec<RulePart>, String)> {
        const OPS: [&str; 6] = ["=", "<>", "<", "<=", ">", ">="];
        let cmp = |kind: &str, field: &str, op: &str, value: &str| -> Option<Vec<RulePart>> {
            OPS.contains(&op).then(|| {
                vec![
                    RulePart::Text(format!(" {kind} ")),
                    RulePart::Field(field.trim().to_owned()),
                    RulePart::Text(format!(" {op} {} ", q(value))),
                ]
            })
        };
        let switches = |default: &str, once: bool| {
            let mut s = String::new();
            if !default.is_empty() {
                s.push_str(&format!(" \\d {}", q(default)));
            }
            if once {
                s.push_str(" \\o");
            }
            s
        };
        Some(match self {
            MergeRule::If {
                field,
                op,
                value,
                then,
                otherwise,
            } => {
                let mut p = cmp("IF", field, op, value)?;
                p.push(RulePart::Text(format!(
                    "{} {} \\* MERGEFORMAT ",
                    q(then),
                    q(otherwise)
                )));
                (p, then.clone())
            }
            MergeRule::SkipIf { field, op, value } => {
                (cmp("SKIPIF", field, op, value)?, String::new())
            }
            MergeRule::NextIf { field, op, value } => {
                (cmp("NEXTIF", field, op, value)?, String::new())
            }
            MergeRule::Next => (vec![RulePart::Text(" NEXT ".into())], String::new()),
            MergeRule::MergeRec => (vec![RulePart::Text(" MERGEREC ".into())], "1".into()),
            MergeRule::MergeSeq => (vec![RulePart::Text(" MERGESEQ ".into())], "1".into()),
            MergeRule::Ask {
                name,
                prompt,
                default,
                once,
            } => (
                vec![RulePart::Text(format!(
                    " ASK {} {}{} ",
                    bookmark_name(name)?,
                    q(prompt),
                    switches(default, *once)
                ))],
                String::new(),
            ),
            MergeRule::FillIn {
                prompt,
                default,
                once,
            } => (
                vec![RulePart::Text(format!(
                    " FILLIN {}{} ",
                    q(prompt),
                    switches(default, *once)
                ))],
                default.clone(),
            ),
            MergeRule::Set { name, value } => (
                vec![RulePart::Text(format!(
                    " SET {} {} ",
                    bookmark_name(name)?,
                    q(value)
                ))],
                String::new(),
            ),
        })
    }
}
