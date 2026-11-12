//! XML tree and an XPath 1.0 subset for `FILTERXML` ([MS-XLSX] future function
//! `_xlfn.FILTERXML`; XPath 1.0, W3C Recommendation 1999).
//!
//! Implemented from the XPath 1.0 recommendation:
//!
//! * location paths (§2): absolute and relative, `//`, `.`, `..`, the axes `child`,
//!   `attribute` (`@`), `descendant`, `descendant-or-self`, `self`, `parent`, `ancestor`,
//!   `ancestor-or-self`, `following-sibling`, `preceding-sibling`; node tests `name`,
//!   `prefix:name`, `*`, `prefix:*`, `text()`, `node()`; predicates (§2.4) with positional
//!   (`[2]`, `[last()]`, `[position()<3]`) and boolean (`[@a='v']`, `[name='v']`) forms;
//! * expressions (§3): `or`, `and`, `=`, `!=`, `<`, `<=`, `>`, `>=`, `+`, `-`, `*`, `div`,
//!   `mod`, unary `-`, `|`, parenthesised filter expressions with predicates;
//! * core functions (§4): `last`, `position`, `count`, `name`, `local-name`, `string`,
//!   `concat`, `starts-with`, `contains`, `substring-before`, `substring-after`,
//!   `substring`, `string-length`, `normalize-space`, `translate`, `boolean`, `not`, `true`,
//!   `false`, `number`, `sum`, `floor`, `ceiling`, `round`.
//!
//! Namespaces (Namespaces in XML 1.0 §6, XPath 1.0 §2.3): every element and attribute name
//! is expanded with the namespace URI its prefix is bound to in scope. A prefixed name test
//! `p:local` / `p:*` resolves `p` with the declarations found in the document itself (the
//! first `xmlns:p` in document order — `FILTERXML` has no other way to supply bindings) and
//! matches by namespace URI + local name, so `//m:v` finds `<a:v xmlns:a="urn:x"/>` when the
//! document also binds `m` to `urn:x`; a prefix the document never declares is matched as
//! written. Unprefixed name tests match the name as written (default-namespace elements
//! included). `namespace-uri()` (§4.1) reports the expanded URI. Namespace declarations are
//! not attribute nodes. Whitespace-only text between elements is dropped, as XML parsers
//! do by default when whitespace is not significant. Variables, comments and processing
//! instructions are not modelled.

use std::cell::Cell;

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

/// Deepest element nesting accepted.
const MAX_NESTING: usize = 256;
/// Deepest expression nesting accepted.
const MAX_EXPR_DEPTH: usize = 64;
/// Upper bound on nodes visited by one evaluation (guards `//*[//*[//*]]` on big documents).
const MAX_VISITS: u64 = 5_000_000;

// ------------------------------------------------------------------- XML tree

enum Kind {
    Root,
    Element {
        name: String,
        /// Namespace URI of the element name (`None`: no namespace).
        ns: Option<String>,
        /// (qualified name, value, namespace URI); unprefixed attributes have no namespace.
        attrs: Vec<(String, String, Option<String>)>,
    },
    Text(String),
}

struct Node {
    kind: Kind,
    parent: usize,
    children: Vec<usize>,
    /// One past the last descendant (nodes are numbered in document order).
    end: usize,
}

struct Doc {
    nodes: Vec<Node>,
    /// First declaration of each prefix in document order (prefix, URI), used to resolve the
    /// prefixes written in the XPath expression.
    prefixes: Vec<(String, String)>,
}

/// In-scope namespace bindings: (prefix, URI) pushed per element; `""` is the default
/// namespace, an empty URI undeclares it (Namespaces in XML 1.0 §6.2).
type Scope = Vec<(String, String)>;

fn lookup<'s>(scope: &'s Scope, prefix: &str) -> Option<&'s str> {
    scope
        .iter()
        .rev()
        .find(|(p, _)| p == prefix)
        .map(|(_, u)| u.as_str())
        .filter(|u| !u.is_empty())
}

/// Namespace URI of a qualified name: the bound prefix, or the default namespace for an
/// unprefixed element name (never for an unprefixed attribute, §6.2).
fn expand(scope: &Scope, qname: &str, is_attr: bool) -> Option<String> {
    match qname.split_once(':') {
        Some(("xml", _)) => Some("http://www.w3.org/XML/1998/namespace".to_owned()),
        Some((p, _)) => lookup(scope, p).map(str::to_owned),
        None if is_attr => None,
        None => lookup(scope, "").map(str::to_owned),
    }
}

/// Build an element node, pushing its namespace declarations onto `scope` (the caller pops
/// `scope` back to its length before the call when the element closes) and recording the
/// first declaration of each prefix in `prefixes`.
fn element(
    e: &BytesStart<'_>,
    parent: usize,
    index: usize,
    scope: &mut Scope,
    prefixes: &mut Vec<(String, String)>,
) -> Option<Node> {
    let name = <_ as AsRef<str>>::as_ref(&e.name()).to_owned();
    let mut raw = Vec::new();
    for a in e.attributes() {
        let a = a.ok()?;
        let key = <_ as AsRef<str>>::as_ref(&a.key).to_owned();
        let value = a
            .normalized_value(XmlVersion::Implicit1_0)
            .ok()?
            .into_owned();
        // Namespace declarations are not attributes in the XPath data model (§5.3).
        if key == "xmlns" {
            scope.push((String::new(), value));
            continue;
        }
        if let Some(p) = key.strip_prefix("xmlns:") {
            if !value.is_empty() && !prefixes.iter().any(|(q, _)| q == p) {
                prefixes.push((p.to_owned(), value.clone()));
            }
            scope.push((p.to_owned(), value));
            continue;
        }
        raw.push((key, value));
    }
    let ns = expand(scope, &name, false);
    let attrs = raw
        .into_iter()
        .map(|(k, v)| {
            let ns = expand(scope, &k, true);
            (k, v, ns)
        })
        .collect();
    Some(Node {
        kind: Kind::Element { name, ns, attrs },
        parent,
        children: Vec::new(),
        end: index + 1,
    })
}

impl Doc {
    fn parse(xml: &str) -> Option<Doc> {
        let mut reader = Reader::from_str(xml);
        let mut doc = Doc {
            nodes: vec![Node {
                kind: Kind::Root,
                parent: 0,
                children: Vec::new(),
                end: 1,
            }],
            prefixes: Vec::new(),
        };
        let mut scope: Scope = Vec::new();
        // Scope length before each open element, popped on its end tag.
        let mut marks: Vec<usize> = Vec::new();
        let mut stack = vec![0usize];
        let mut roots = 0;
        loop {
            match reader.read_event().ok()? {
                Event::Start(e) => {
                    let top = doc.open_child(&mut roots, &stack)?;
                    let idx = doc.nodes.len();
                    marks.push(scope.len());
                    let node = element(&e, top, idx, &mut scope, &mut doc.prefixes)?;
                    doc.nodes.push(node);
                    doc.nodes[top].children.push(idx);
                    stack.push(idx);
                    if stack.len() > MAX_NESTING {
                        return None;
                    }
                }
                Event::Empty(e) => {
                    let top = doc.open_child(&mut roots, &stack)?;
                    let idx = doc.nodes.len();
                    let mark = scope.len();
                    let node = element(&e, top, idx, &mut scope, &mut doc.prefixes)?;
                    scope.truncate(mark);
                    doc.nodes.push(node);
                    doc.nodes[top].children.push(idx);
                }
                Event::End(_) => {
                    scope.truncate(marks.pop()?);
                    let idx = stack.pop()?;
                    if stack.is_empty() {
                        return None;
                    }
                    doc.drop_blank_text(idx);
                    doc.nodes[idx].end = doc.nodes.len();
                }
                Event::Text(t) => doc.text(&stack, &t.xml10_content())?,
                Event::CData(t) => doc.text(&stack, &t.xml10_content())?,
                Event::GeneralRef(g) => {
                    let c = match g.resolve_char_ref().ok()? {
                        Some(c) => c,
                        None => match &*g.xml10_content() {
                            "amp" => '&',
                            "lt" => '<',
                            "gt" => '>',
                            "quot" => '"',
                            "apos" => '\'',
                            _ => return None,
                        },
                    };
                    doc.text(&stack, c.encode_utf8(&mut [0; 4]))?;
                }
                Event::Eof => break,
                _ => {}
            }
        }
        if stack.len() != 1 || roots != 1 {
            return None;
        }
        doc.nodes[0].end = doc.nodes.len();
        Some(doc)
    }

    /// Prepare to add an element under the open element; only one document element.
    fn open_child(&mut self, roots: &mut usize, stack: &[usize]) -> Option<usize> {
        let top = *stack.last()?;
        if stack.len() == 1 {
            *roots += 1;
            if *roots > 1 {
                return None;
            }
        }
        self.drop_blank_text(top);
        Some(top)
    }

    /// Remove a trailing whitespace-only text child of `parent` (it is the newest node).
    fn drop_blank_text(&mut self, parent: usize) {
        let last = self.nodes.len() - 1;
        if self.nodes[parent].children.last() == Some(&last)
            && matches!(&self.nodes[last].kind, Kind::Text(t) if t.trim().is_empty())
        {
            self.nodes.pop();
            self.nodes[parent].children.pop();
        }
    }

    fn text(&mut self, stack: &[usize], s: &str) -> Option<()> {
        let top = *stack.last()?;
        if stack.len() == 1 {
            // Only whitespace may stand outside the document element.
            return s.trim().is_empty().then_some(());
        }
        let last = self.nodes.len() - 1;
        if self.nodes[top].children.last() == Some(&last) {
            if let Kind::Text(t) = &mut self.nodes[last].kind {
                t.push_str(s);
                return Some(());
            }
        }
        let idx = self.nodes.len();
        self.nodes.push(Node {
            kind: Kind::Text(s.to_owned()),
            parent: top,
            children: Vec::new(),
            end: idx + 1,
        });
        self.nodes[top].children.push(idx);
        Some(())
    }

    fn name(&self, item: Item) -> &str {
        match item {
            Item::Node(i) => match &self.nodes[i].kind {
                Kind::Element { name, .. } => name,
                _ => "",
            },
            Item::Attr(e, a) => match &self.nodes[e].kind {
                Kind::Element { attrs, .. } => &attrs[a].0,
                _ => "",
            },
        }
    }

    /// Expanded namespace URI of an element or attribute (`None`: no namespace).
    fn namespace(&self, item: Item) -> Option<&str> {
        match item {
            Item::Node(i) => match &self.nodes[i].kind {
                Kind::Element { ns, .. } => ns.as_deref(),
                _ => None,
            },
            Item::Attr(e, a) => match &self.nodes[e].kind {
                Kind::Element { attrs, .. } => attrs[a].2.as_deref(),
                _ => None,
            },
        }
    }

    /// URI the document binds `prefix` to (first declaration in document order).
    fn prefix_uri(&self, prefix: &str) -> Option<&str> {
        self.prefixes
            .iter()
            .find(|(p, _)| p == prefix)
            .map(|(_, u)| u.as_str())
    }

    /// Whether `item`'s name matches the qualified name test `qname` (§2.3): `p:local`
    /// with a prefix the document declares compares namespace URI + local name.
    fn name_matches(&self, item: Item, qname: &str) -> bool {
        let written = self.name(item);
        if let Some((p, local)) = qname.split_once(':') {
            if let Some(uri) = self.prefix_uri(p) {
                let own_local = written.rsplit(':').next().unwrap_or(written);
                return own_local == local && self.namespace(item) == Some(uri);
            }
        }
        written == qname
    }

    /// Whether `item` matches the `prefix:*` test.
    fn prefix_matches(&self, item: Item, prefix: &str) -> bool {
        match self.prefix_uri(prefix) {
            Some(uri) => self.namespace(item) == Some(uri),
            None => self
                .name(item)
                .split_once(':')
                .is_some_and(|(q, _)| q == prefix),
        }
    }

    /// String-value (§5): text of a text node, value of an attribute, concatenated
    /// descendant text of the root and of elements.
    fn string_value(&self, item: Item) -> String {
        match item {
            Item::Node(i) => {
                if let Kind::Text(t) = &self.nodes[i].kind {
                    return t.clone();
                }
                let mut s = String::new();
                for n in &self.nodes[i + 1..self.nodes[i].end] {
                    if let Kind::Text(t) = &n.kind {
                        s.push_str(t);
                    }
                }
                s
            }
            Item::Attr(e, a) => match &self.nodes[e].kind {
                Kind::Element { attrs, .. } => attrs[a].1.clone(),
                _ => String::new(),
            },
        }
    }
}

// ------------------------------------------------------------------- lexer

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Slash,
    DSlash,
    LBr,
    RBr,
    LPar,
    RPar,
    At,
    Comma,
    Pipe,
    Dot,
    DDot,
    /// `*` as a name test.
    Star,
    /// An operator: `= != < <= > >= + - * and or div mod`.
    Op(&'static str),
    Num(f64),
    Str(String),
    /// A name test (`name`, `p:name`, `p:*`).
    Name(String),
    /// An axis name followed by `::`.
    Axis(String),
    /// A function name followed by `(`.
    Func(String),
    /// `text` / `node` / `comment` / `processing-instruction` followed by `(`.
    NodeType(String),
}

fn is_name_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '\u{B7}')
}

/// Tokenize (§3.7), with the disambiguation rule: after an operand, `*` is multiplication and
/// a name is an operator name.
fn lex(src: &str) -> Option<Vec<Tok>> {
    let chars: Vec<char> = src.chars().collect();
    let mut out: Vec<Tok> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let after_operand = matches!(
            out.last(),
            Some(
                Tok::RPar
                    | Tok::RBr
                    | Tok::Num(_)
                    | Tok::Str(_)
                    | Tok::Name(_)
                    | Tok::Star
                    | Tok::Dot
                    | Tok::DDot
            )
        );
        let next = chars.get(i + 1).copied();
        match c {
            ' ' | '\t' | '\r' | '\n' => i += 1,
            '/' if next == Some('/') => {
                out.push(Tok::DSlash);
                i += 2;
            }
            '/' => {
                out.push(Tok::Slash);
                i += 1;
            }
            '[' => {
                out.push(Tok::LBr);
                i += 1;
            }
            ']' => {
                out.push(Tok::RBr);
                i += 1;
            }
            '(' => {
                out.push(Tok::LPar);
                i += 1;
            }
            ')' => {
                out.push(Tok::RPar);
                i += 1;
            }
            '@' => {
                out.push(Tok::At);
                i += 1;
            }
            ',' => {
                out.push(Tok::Comma);
                i += 1;
            }
            '|' => {
                out.push(Tok::Pipe);
                i += 1;
            }
            '.' if next == Some('.') => {
                out.push(Tok::DDot);
                i += 2;
            }
            '.' if !next.is_some_and(|d| d.is_ascii_digit()) => {
                out.push(Tok::Dot);
                i += 1;
            }
            '*' => {
                out.push(if after_operand {
                    Tok::Op("*")
                } else {
                    Tok::Star
                });
                i += 1;
            }
            '=' => {
                out.push(Tok::Op("="));
                i += 1;
            }
            '!' if next == Some('=') => {
                out.push(Tok::Op("!="));
                i += 2;
            }
            '<' | '>' => {
                let eq = next == Some('=');
                out.push(Tok::Op(match (c, eq) {
                    ('<', true) => "<=",
                    ('<', false) => "<",
                    (_, true) => ">=",
                    _ => ">",
                }));
                i += if eq { 2 } else { 1 };
            }
            '+' => {
                out.push(Tok::Op("+"));
                i += 1;
            }
            '-' => {
                out.push(Tok::Op("-"));
                i += 1;
            }
            '"' | '\'' => {
                let end = chars[i + 1..].iter().position(|&d| d == c)? + i + 1;
                out.push(Tok::Str(chars[i + 1..end].iter().collect()));
                i = end + 1;
            }
            d if d.is_ascii_digit() || d == '.' => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                if i < chars.len() && chars[i] == '.' {
                    i += 1;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                let s: String = chars[start..i].iter().collect();
                out.push(Tok::Num(s.parse().ok()?));
            }
            d if is_name_start(d) => {
                let start = i;
                while i < chars.len() && is_name_char(chars[i]) {
                    i += 1;
                }
                // `prefix:local` or `prefix:*` (but not the axis separator `::`).
                if i + 1 < chars.len() && chars[i] == ':' && chars[i + 1] != ':' {
                    if chars[i + 1] == '*' {
                        i += 2;
                    } else if is_name_start(chars[i + 1]) {
                        i += 1;
                        while i < chars.len() && is_name_char(chars[i]) {
                            i += 1;
                        }
                    }
                }
                let name: String = chars[start..i].iter().collect();
                if after_operand {
                    out.push(Tok::Op(match name.as_str() {
                        "and" => "and",
                        "or" => "or",
                        "div" => "div",
                        "mod" => "mod",
                        _ => return None,
                    }));
                    continue;
                }
                let mut j = i;
                while j < chars.len() && chars[j].is_whitespace() {
                    j += 1;
                }
                if j + 1 < chars.len() && chars[j] == ':' && chars[j + 1] == ':' {
                    out.push(Tok::Axis(name));
                    i = j + 2;
                } else if j < chars.len() && chars[j] == '(' && !name.ends_with('*') {
                    if matches!(
                        name.as_str(),
                        "text" | "node" | "comment" | "processing-instruction"
                    ) {
                        out.push(Tok::NodeType(name));
                    } else {
                        out.push(Tok::Func(name));
                    }
                    i = j + 1;
                } else {
                    out.push(Tok::Name(name));
                }
            }
            _ => return None,
        }
    }
    Some(out)
}

// ------------------------------------------------------------------- parser

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Axis {
    Child,
    Attribute,
    Descendant,
    DescendantOrSelf,
    SelfNode,
    Parent,
    Ancestor,
    AncestorOrSelf,
    FollowingSibling,
    PrecedingSibling,
}

impl Axis {
    fn from_name(s: &str) -> Option<Axis> {
        Some(match s {
            "child" => Axis::Child,
            "attribute" => Axis::Attribute,
            "descendant" => Axis::Descendant,
            "descendant-or-self" => Axis::DescendantOrSelf,
            "self" => Axis::SelfNode,
            "parent" => Axis::Parent,
            "ancestor" => Axis::Ancestor,
            "ancestor-or-self" => Axis::AncestorOrSelf,
            "following-sibling" => Axis::FollowingSibling,
            "preceding-sibling" => Axis::PrecedingSibling,
            _ => return None,
        })
    }
}

#[derive(Debug)]
enum Test {
    Name(String),
    Any,
    Prefix(String),
    Text,
    Node,
}

#[derive(Debug)]
struct Step {
    axis: Axis,
    test: Test,
    preds: Vec<Expr>,
}

#[derive(Debug)]
enum Start {
    Root,
    Context,
    Filter(Box<Expr>, Vec<Expr>),
}

#[derive(Debug)]
enum Expr {
    Binary(&'static str, Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),
    Union(Box<Expr>, Box<Expr>),
    Num(f64),
    Str(String),
    Func(String, Vec<Expr>),
    Path(Start, Vec<Step>),
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == Some(t) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn eat_op(&mut self, ops: &[&'static str]) -> Option<&'static str> {
        if let Some(Tok::Op(o)) = self.peek() {
            if let Some(found) = ops.iter().find(|x| *x == o) {
                self.pos += 1;
                return Some(found);
            }
        }
        None
    }

    fn expr(&mut self) -> Option<Expr> {
        self.depth += 1;
        if self.depth > MAX_EXPR_DEPTH {
            return None;
        }
        let e = self.binary(0);
        self.depth -= 1;
        e
    }

    /// Binary operators by precedence level (§3.4, §3.5): or, and, equality, relational,
    /// additive, multiplicative.
    fn binary(&mut self, level: usize) -> Option<Expr> {
        const LEVELS: [&[&str]; 6] = [
            &["or"],
            &["and"],
            &["=", "!="],
            &["<", "<=", ">", ">="],
            &["+", "-"],
            &["*", "div", "mod"],
        ];
        if level == LEVELS.len() {
            return self.unary();
        }
        let mut left = self.binary(level + 1)?;
        while let Some(op) = self.eat_op(LEVELS[level]) {
            let right = self.binary(level + 1)?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Some(left)
    }

    fn unary(&mut self) -> Option<Expr> {
        if self.eat_op(&["-"]).is_some() {
            self.depth += 1;
            if self.depth > MAX_EXPR_DEPTH {
                return None;
            }
            let inner = self.unary()?;
            self.depth -= 1;
            return Some(Expr::Neg(Box::new(inner)));
        }
        let mut left = self.path_expr()?;
        while self.eat(&Tok::Pipe) {
            let right = self.path_expr()?;
            left = Expr::Union(Box::new(left), Box::new(right));
        }
        Some(left)
    }

    fn starts_step(&self) -> bool {
        matches!(
            self.peek(),
            Some(
                Tok::Name(_)
                    | Tok::Star
                    | Tok::At
                    | Tok::Axis(_)
                    | Tok::NodeType(_)
                    | Tok::Dot
                    | Tok::DDot
            )
        )
    }

    fn path_expr(&mut self) -> Option<Expr> {
        match self.peek()? {
            Tok::Slash => {
                self.pos += 1;
                let steps = if self.starts_step() {
                    self.relative(Vec::new())?
                } else {
                    Vec::new()
                };
                Some(Expr::Path(Start::Root, steps))
            }
            Tok::DSlash => {
                self.pos += 1;
                let steps = self.relative(vec![descendant_or_self()])?;
                Some(Expr::Path(Start::Root, steps))
            }
            _ if self.starts_step() => {
                let steps = self.relative(Vec::new())?;
                Some(Expr::Path(Start::Context, steps))
            }
            _ => {
                let primary = self.primary()?;
                let mut preds = Vec::new();
                while self.eat(&Tok::LBr) {
                    preds.push(self.expr()?);
                    if !self.eat(&Tok::RBr) {
                        return None;
                    }
                }
                let mut steps = Vec::new();
                if self.eat(&Tok::Slash) {
                    steps = self.relative(steps)?;
                } else if self.eat(&Tok::DSlash) {
                    steps = self.relative(vec![descendant_or_self()])?;
                }
                if preds.is_empty() && steps.is_empty() {
                    Some(primary)
                } else {
                    Some(Expr::Path(Start::Filter(Box::new(primary), preds), steps))
                }
            }
        }
    }

    fn primary(&mut self) -> Option<Expr> {
        match self.peek()?.clone() {
            Tok::LPar => {
                self.pos += 1;
                let e = self.expr()?;
                self.eat(&Tok::RPar).then_some(e)
            }
            Tok::Num(n) => {
                self.pos += 1;
                Some(Expr::Num(n))
            }
            Tok::Str(s) => {
                self.pos += 1;
                Some(Expr::Str(s))
            }
            Tok::Func(name) => {
                self.pos += 1;
                let mut args = Vec::new();
                if !self.eat(&Tok::RPar) {
                    loop {
                        args.push(self.expr()?);
                        if self.eat(&Tok::RPar) {
                            break;
                        }
                        if !self.eat(&Tok::Comma) {
                            return None;
                        }
                    }
                }
                Some(Expr::Func(name, args))
            }
            _ => None,
        }
    }

    /// `Step (('/' | '//') Step)*` appended to `steps`.
    fn relative(&mut self, mut steps: Vec<Step>) -> Option<Vec<Step>> {
        loop {
            steps.push(self.step()?);
            if self.eat(&Tok::Slash) {
                continue;
            }
            if self.eat(&Tok::DSlash) {
                steps.push(descendant_or_self());
                continue;
            }
            return Some(steps);
        }
    }

    fn step(&mut self) -> Option<Step> {
        if self.eat(&Tok::Dot) {
            return Some(Step {
                axis: Axis::SelfNode,
                test: Test::Node,
                preds: Vec::new(),
            });
        }
        if self.eat(&Tok::DDot) {
            return Some(Step {
                axis: Axis::Parent,
                test: Test::Node,
                preds: Vec::new(),
            });
        }
        let axis = match self.peek()?.clone() {
            Tok::At => {
                self.pos += 1;
                Axis::Attribute
            }
            Tok::Axis(name) => {
                self.pos += 1;
                Axis::from_name(&name)?
            }
            _ => Axis::Child,
        };
        let test = match self.peek()?.clone() {
            Tok::Star => Test::Any,
            Tok::Name(n) => match n.strip_suffix(":*") {
                Some(p) => Test::Prefix(p.to_owned()),
                None => Test::Name(n),
            },
            Tok::NodeType(t) => {
                self.pos += 1;
                if !self.eat(&Tok::RPar) {
                    return None;
                }
                self.pos -= 1;
                match t.as_str() {
                    "text" => Test::Text,
                    "node" => Test::Node,
                    // Comments and processing instructions are not modelled: never match.
                    _ => Test::Name(String::new()),
                }
            }
            _ => return None,
        };
        self.pos += 1;
        let mut preds = Vec::new();
        while self.eat(&Tok::LBr) {
            preds.push(self.expr()?);
            if !self.eat(&Tok::RBr) {
                return None;
            }
        }
        Some(Step { axis, test, preds })
    }
}

fn descendant_or_self() -> Step {
    Step {
        axis: Axis::DescendantOrSelf,
        test: Test::Node,
        preds: Vec::new(),
    }
}

fn parse(src: &str) -> Option<Expr> {
    let mut p = Parser {
        toks: lex(src)?,
        pos: 0,
        depth: 0,
    };
    let e = p.expr()?;
    (p.pos == p.toks.len()).then_some(e)
}

// ------------------------------------------------------------------- evaluation

/// A node: tree node by index, or attribute `(element, index)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Item {
    Node(usize),
    Attr(usize, usize),
}

impl Item {
    /// Document order: attributes follow their element and precede its children.
    fn order(self) -> (usize, usize) {
        match self {
            Item::Node(i) => (i, 0),
            Item::Attr(e, a) => (e, a + 1),
        }
    }
}

enum Val {
    Nodes(Vec<Item>),
    Num(f64),
    Str(String),
    Bool(bool),
}

#[derive(Clone, Copy)]
struct Focus {
    item: Item,
    pos: usize,
    size: usize,
}

struct Eval<'d> {
    doc: &'d Doc,
    visits: Cell<u64>,
}

fn sort_nodes(v: &mut Vec<Item>) {
    v.sort_by_key(|i| i.order());
    v.dedup();
}

/// XPath number → string (§4.2 `string`).
fn num_to_string(n: f64) -> String {
    if n.is_nan() {
        "NaN".to_owned()
    } else if n.is_infinite() {
        if n > 0.0 { "Infinity" } else { "-Infinity" }.to_owned()
    } else if n == 0.0 {
        "0".to_owned()
    } else {
        format!("{n}")
    }
}

/// String → number (§4.4 `number`): optional whitespace, optional `-`, digits with an optional
/// fraction; anything else is NaN.
fn str_to_num(s: &str) -> f64 {
    let t = s.trim();
    let body = t.strip_prefix('-').unwrap_or(t);
    let valid = !body.is_empty()
        && body.chars().all(|c| c.is_ascii_digit() || c == '.')
        && body.chars().filter(|&c| c == '.').count() <= 1
        && body.chars().any(|c| c.is_ascii_digit());
    if valid {
        t.parse().unwrap_or(f64::NAN)
    } else {
        f64::NAN
    }
}

/// XPath 1.0 `round`: nearest integer, halves towards positive infinity.
fn xpath_round(n: f64) -> f64 {
    if n.is_finite() {
        (n + 0.5).floor()
    } else {
        n
    }
}

impl Eval<'_> {
    fn charge(&self, n: u64) -> Option<()> {
        let v = self.visits.get() + n;
        self.visits.set(v);
        (v <= MAX_VISITS).then_some(())
    }

    fn string(&self, v: &Val) -> String {
        match v {
            Val::Nodes(n) => n
                .first()
                .map(|&i| self.doc.string_value(i))
                .unwrap_or_default(),
            Val::Num(n) => num_to_string(*n),
            Val::Str(s) => s.clone(),
            Val::Bool(b) => if *b { "true" } else { "false" }.to_owned(),
        }
    }

    fn number(&self, v: &Val) -> f64 {
        match v {
            Val::Num(n) => *n,
            Val::Bool(b) => f64::from(u8::from(*b)),
            other => str_to_num(&self.string(other)),
        }
    }

    fn boolean(v: &Val) -> bool {
        match v {
            Val::Nodes(n) => !n.is_empty(),
            Val::Num(n) => *n != 0.0 && !n.is_nan(),
            Val::Str(s) => !s.is_empty(),
            Val::Bool(b) => *b,
        }
    }

    fn eval(&self, e: &Expr, f: Focus) -> Option<Val> {
        Some(match e {
            Expr::Num(n) => Val::Num(*n),
            Expr::Str(s) => Val::Str(s.clone()),
            Expr::Neg(x) => Val::Num(-self.number(&self.eval(x, f)?)),
            Expr::Union(a, b) => {
                let (Val::Nodes(mut x), Val::Nodes(y)) = (self.eval(a, f)?, self.eval(b, f)?)
                else {
                    return None;
                };
                x.extend(y);
                sort_nodes(&mut x);
                Val::Nodes(x)
            }
            Expr::Binary(op, a, b) => match *op {
                "or" => {
                    Val::Bool(Self::boolean(&self.eval(a, f)?) || Self::boolean(&self.eval(b, f)?))
                }
                "and" => {
                    Val::Bool(Self::boolean(&self.eval(a, f)?) && Self::boolean(&self.eval(b, f)?))
                }
                "=" | "!=" | "<" | "<=" | ">" | ">=" => {
                    Val::Bool(self.compare(op, &self.eval(a, f)?, &self.eval(b, f)?))
                }
                _ => {
                    let x = self.number(&self.eval(a, f)?);
                    let y = self.number(&self.eval(b, f)?);
                    Val::Num(match *op {
                        "+" => x + y,
                        "-" => x - y,
                        "*" => x * y,
                        "div" => x / y,
                        _ => x % y,
                    })
                }
            },
            Expr::Func(name, args) => self.call(name, args, f)?,
            Expr::Path(start, steps) => {
                let mut set = match start {
                    Start::Root => vec![Item::Node(0)],
                    Start::Context => vec![f.item],
                    Start::Filter(primary, preds) => {
                        let Val::Nodes(mut set) = self.eval(primary, f)? else {
                            return None;
                        };
                        for p in preds {
                            set = self.filter(set, p)?;
                        }
                        set
                    }
                };
                for step in steps {
                    set = self.step(&set, step)?;
                }
                Val::Nodes(set)
            }
        })
    }

    /// Comparisons (§3.4), including the existential node-set rules.
    fn compare(&self, op: &str, a: &Val, b: &Val) -> bool {
        let cmp_num = |x: f64, y: f64| match op {
            "=" => x == y,
            "!=" => x != y,
            "<" => x < y,
            "<=" => x <= y,
            ">" => x > y,
            _ => x >= y,
        };
        let eq_op = op == "=" || op == "!=";
        let cmp_str = |x: &str, y: &str| {
            if eq_op {
                (x == y) == (op == "=")
            } else {
                cmp_num(str_to_num(x), str_to_num(y))
            }
        };
        match (a, b) {
            (Val::Nodes(x), Val::Nodes(y)) => {
                let ys: Vec<String> = y.iter().map(|&i| self.doc.string_value(i)).collect();
                x.iter().any(|&i| {
                    let sx = self.doc.string_value(i);
                    ys.iter().any(|sy| cmp_str(&sx, sy))
                })
            }
            (Val::Nodes(x), other) | (other, Val::Nodes(x)) => {
                // Keep operand order for relational operators.
                let nodes_left = matches!(a, Val::Nodes(_));
                match other {
                    Val::Bool(bv) => {
                        let nb = !x.is_empty();
                        let (l, r) = if nodes_left { (nb, *bv) } else { (*bv, nb) };
                        cmp_num(f64::from(u8::from(l)), f64::from(u8::from(r)))
                    }
                    Val::Num(n) => x.iter().any(|&i| {
                        let v = str_to_num(&self.doc.string_value(i));
                        if nodes_left {
                            cmp_num(v, *n)
                        } else {
                            cmp_num(*n, v)
                        }
                    }),
                    _ => {
                        let s = self.string(other);
                        x.iter().any(|&i| {
                            let v = self.doc.string_value(i);
                            if nodes_left {
                                cmp_str(&v, &s)
                            } else {
                                cmp_str(&s, &v)
                            }
                        })
                    }
                }
            }
            _ if eq_op => {
                let eq = if matches!(a, Val::Bool(_)) || matches!(b, Val::Bool(_)) {
                    Self::boolean(a) == Self::boolean(b)
                } else if matches!(a, Val::Num(_)) || matches!(b, Val::Num(_)) {
                    self.number(a) == self.number(b)
                } else {
                    self.string(a) == self.string(b)
                };
                eq == (op == "=")
            }
            _ => cmp_num(self.number(a), self.number(b)),
        }
    }

    /// Filter a node set (in document order) by one predicate (§2.4).
    fn filter(&self, set: Vec<Item>, pred: &Expr) -> Option<Vec<Item>> {
        let size = set.len();
        let mut out = Vec::new();
        for (k, item) in set.into_iter().enumerate() {
            let focus = Focus {
                item,
                pos: k + 1,
                size,
            };
            let keep = match self.eval(pred, focus)? {
                Val::Num(n) => n == (k + 1) as f64,
                v => Self::boolean(&v),
            };
            if keep {
                out.push(item);
            }
        }
        Some(out)
    }

    fn step(&self, set: &[Item], step: &Step) -> Option<Vec<Item>> {
        let mut out = Vec::new();
        for &ctx in set {
            let mut cands: Vec<Item> = self
                .axis(ctx, step.axis)
                .into_iter()
                .filter(|&i| self.test(i, step))
                .collect();
            self.charge(cands.len() as u64 + 1)?;
            // Candidates are in axis order, so positions count along the axis.
            for p in &step.preds {
                cands = self.filter(cands, p)?;
            }
            out.extend(cands);
        }
        sort_nodes(&mut out);
        Some(out)
    }

    fn axis(&self, item: Item, axis: Axis) -> Vec<Item> {
        let nodes = &self.doc.nodes;
        let parent = |it: Item| match it {
            Item::Node(0) => None,
            Item::Node(i) => Some(nodes[i].parent),
            Item::Attr(e, _) => Some(e),
        };
        let ancestors = |it: Item| {
            let mut v = Vec::new();
            let mut cur = parent(it);
            while let Some(p) = cur {
                v.push(Item::Node(p));
                cur = parent(Item::Node(p));
            }
            v
        };
        match (axis, item) {
            (Axis::SelfNode, _) => vec![item],
            (Axis::Parent, _) => parent(item).map(Item::Node).into_iter().collect(),
            (Axis::Ancestor, _) => ancestors(item),
            (Axis::AncestorOrSelf, _) => {
                let mut v = vec![item];
                v.extend(ancestors(item));
                v
            }
            (_, Item::Attr(..))
            | (Axis::FollowingSibling | Axis::PrecedingSibling, Item::Node(0)) => Vec::new(),
            (Axis::Child, Item::Node(i)) => {
                nodes[i].children.iter().map(|&c| Item::Node(c)).collect()
            }
            (Axis::Attribute, Item::Node(i)) => match &nodes[i].kind {
                Kind::Element { attrs, .. } => (0..attrs.len()).map(|a| Item::Attr(i, a)).collect(),
                _ => Vec::new(),
            },
            (Axis::Descendant, Item::Node(i)) => (i + 1..nodes[i].end).map(Item::Node).collect(),
            (Axis::DescendantOrSelf, Item::Node(i)) => (i..nodes[i].end).map(Item::Node).collect(),
            (Axis::FollowingSibling, Item::Node(i)) => {
                let sib = &nodes[nodes[i].parent].children;
                let k = sib.iter().position(|&c| c == i).unwrap_or(sib.len());
                sib[k + 1..].iter().map(|&c| Item::Node(c)).collect()
            }
            (Axis::PrecedingSibling, Item::Node(i)) => {
                let sib = &nodes[nodes[i].parent].children;
                let k = sib.iter().position(|&c| c == i).unwrap_or(0);
                sib[..k].iter().rev().map(|&c| Item::Node(c)).collect()
            }
        }
    }

    /// Node test (§2.3); the principal node type is attribute on the attribute axis,
    /// element otherwise.
    fn test(&self, item: Item, step: &Step) -> bool {
        let principal = match item {
            Item::Attr(..) => step.axis == Axis::Attribute,
            Item::Node(i) => {
                step.axis != Axis::Attribute
                    && matches!(self.doc.nodes[i].kind, Kind::Element { .. })
            }
        };
        match &step.test {
            Test::Node => true,
            Test::Text => {
                matches!(item, Item::Node(i) if matches!(self.doc.nodes[i].kind, Kind::Text(_)))
            }
            Test::Any => principal,
            Test::Name(n) => principal && self.doc.name_matches(item, n),
            Test::Prefix(p) => principal && self.doc.prefix_matches(item, p),
        }
    }

    fn call(&self, name: &str, args: &[Expr], f: Focus) -> Option<Val> {
        let arity = |lo: usize, hi: usize| (lo..=hi).contains(&args.len()).then_some(());
        let arg = |k: usize| self.eval(&args[k], f);
        let str_arg = |k: usize| -> Option<String> {
            if k < args.len() {
                Some(self.string(&arg(k)?))
            } else {
                Some(self.doc.string_value(f.item))
            }
        };
        let nodes_arg = |k: usize| -> Option<Vec<Item>> {
            if k < args.len() {
                match arg(k)? {
                    Val::Nodes(n) => Some(n),
                    _ => None,
                }
            } else {
                Some(vec![f.item])
            }
        };
        Some(match name {
            "last" => {
                arity(0, 0)?;
                Val::Num(f.size as f64)
            }
            "position" => {
                arity(0, 0)?;
                Val::Num(f.pos as f64)
            }
            "count" => {
                arity(1, 1)?;
                Val::Num(nodes_arg(0)?.len() as f64)
            }
            "name" | "local-name" => {
                arity(0, 1)?;
                let n = nodes_arg(0)?;
                let full = n.first().map(|&i| self.doc.name(i)).unwrap_or_default();
                let shown = if name == "local-name" {
                    full.rsplit(':').next().unwrap_or(full)
                } else {
                    full
                };
                Val::Str(shown.to_owned())
            }
            "namespace-uri" => {
                arity(0, 1)?;
                let n = nodes_arg(0)?;
                let uri = n.first().and_then(|&i| self.doc.namespace(i));
                Val::Str(uri.unwrap_or_default().to_owned())
            }
            "string" => {
                arity(0, 1)?;
                Val::Str(str_arg(0)?)
            }
            "concat" => {
                if args.len() < 2 {
                    return None;
                }
                let mut s = String::new();
                for k in 0..args.len() {
                    s.push_str(&str_arg(k)?);
                }
                Val::Str(s)
            }
            "starts-with" | "contains" | "substring-before" | "substring-after" => {
                arity(2, 2)?;
                let (a, b) = (str_arg(0)?, str_arg(1)?);
                match name {
                    "starts-with" => Val::Bool(a.starts_with(&b)),
                    "contains" => Val::Bool(a.contains(&b)),
                    "substring-before" => {
                        Val::Str(a.find(&b).map(|k| a[..k].to_owned()).unwrap_or_default())
                    }
                    _ => Val::Str(
                        a.find(&b)
                            .map(|k| a[k + b.len()..].to_owned())
                            .unwrap_or_default(),
                    ),
                }
            }
            "substring" => {
                arity(2, 3)?;
                let s = str_arg(0)?;
                let start = xpath_round(self.number(&arg(1)?));
                let len = if args.len() == 3 {
                    xpath_round(self.number(&arg(2)?))
                } else {
                    f64::INFINITY
                };
                // Characters at positions p with start <= p < start + len (1-based).
                let out: String = s
                    .chars()
                    .enumerate()
                    .filter(|&(k, _)| {
                        let p = (k + 1) as f64;
                        p >= start && p < start + len
                    })
                    .map(|(_, c)| c)
                    .collect();
                Val::Str(out)
            }
            "string-length" => {
                arity(0, 1)?;
                Val::Num(str_arg(0)?.chars().count() as f64)
            }
            "normalize-space" => {
                arity(0, 1)?;
                Val::Str(str_arg(0)?.split_whitespace().collect::<Vec<_>>().join(" "))
            }
            "translate" => {
                arity(3, 3)?;
                let (s, from, to) = (str_arg(0)?, str_arg(1)?, str_arg(2)?);
                let from: Vec<char> = from.chars().collect();
                let to: Vec<char> = to.chars().collect();
                let out: String = s
                    .chars()
                    .filter_map(|c| match from.iter().position(|&x| x == c) {
                        Some(k) => to.get(k).copied(),
                        None => Some(c),
                    })
                    .collect();
                Val::Str(out)
            }
            "boolean" => {
                arity(1, 1)?;
                Val::Bool(Self::boolean(&arg(0)?))
            }
            "not" => {
                arity(1, 1)?;
                Val::Bool(!Self::boolean(&arg(0)?))
            }
            "true" | "false" => {
                arity(0, 0)?;
                Val::Bool(name == "true")
            }
            "number" => {
                arity(0, 1)?;
                if args.is_empty() {
                    Val::Num(str_to_num(&self.doc.string_value(f.item)))
                } else {
                    Val::Num(self.number(&arg(0)?))
                }
            }
            "sum" => {
                arity(1, 1)?;
                let n = nodes_arg(0)?;
                Val::Num(
                    n.iter()
                        .map(|&i| str_to_num(&self.doc.string_value(i)))
                        .sum(),
                )
            }
            "floor" | "ceiling" | "round" => {
                arity(1, 1)?;
                let x = self.number(&arg(0)?);
                Val::Num(match name {
                    "floor" => x.floor(),
                    "ceiling" => x.ceil(),
                    _ => xpath_round(x),
                })
            }
            _ => return None,
        })
    }
}

/// Evaluate `path` over `xml`: the string values of the selected nodes in document order.
/// `None` when the XML or the XPath is invalid, or the XPath does not select nodes.
pub(crate) fn filter_xml(xml: &str, path: &str) -> Option<Vec<String>> {
    let doc = Doc::parse(xml)?;
    let expr = parse(path)?;
    let ev = Eval {
        doc: &doc,
        visits: Cell::new(0),
    };
    let focus = Focus {
        item: Item::Node(0),
        pos: 1,
        size: 1,
    };
    match ev.eval(&expr, focus)? {
        Val::Nodes(n) => Some(n.into_iter().map(|i| doc.string_value(i)).collect()),
        _ => None,
    }
}

/// A node value that reads as a decimal number (`12`, `-3.5`, `1e3`) becomes a number.
pub(crate) fn as_number(s: &str) -> Option<f64> {
    let t = s.trim();
    let ok = !t.is_empty()
        && t.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '.' | 'e' | 'E'))
        && t.chars().any(|c| c.is_ascii_digit());
    if !ok {
        return None;
    }
    t.parse::<f64>().ok().filter(|n| n.is_finite())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(xml: &str, path: &str) -> Option<Vec<String>> {
        filter_xml(xml, path)
    }

    const X: &str = "<r><a id=\"1\">x</a><a id=\"2\"><b>y</b>z</a></r>";

    #[test]
    fn paths() {
        assert_eq!(q(X, "/r/a").unwrap(), ["x", "yz"]);
        assert_eq!(q(X, "//a/@id").unwrap(), ["1", "2"]);
        assert_eq!(q(X, "//a[2]/text()").unwrap(), ["z"]);
        assert_eq!(q(X, "//*[@id='2']/b").unwrap(), ["y"]);
        assert_eq!(q(X, "(//a)[last()]").unwrap(), ["yz"]);
        assert_eq!(q(X, "//b/..//@id").unwrap(), ["2"]);
        assert_eq!(q(X, "count(//a)"), None);
        assert_eq!(q(X, "//a[").map(|_| ()), None);
        assert_eq!(q("<r><a></r>", "//a"), None);
        assert_eq!(q("<r/><s/>", "//r"), None);
    }

    #[test]
    fn namespaces_resolve_through_document_declarations() {
        // Same URI under two prefixes: `m:v` matches `a:v` (Namespaces in XML §6).
        let x = "<r xmlns:m=\"urn:x\"><a:v xmlns:a=\"urn:x\">1</a:v><m:v>2</m:v><b:v xmlns:b=\"urn:y\">3</b:v></r>";
        assert_eq!(q(x, "//m:v").unwrap(), ["1", "2"]);
        assert_eq!(q(x, "//b:v").unwrap(), ["3"]);
        assert_eq!(q(x, "//m:*").unwrap(), ["1", "2"]);
        assert_eq!(q(x, "//a:v").unwrap(), ["1", "2"]);
        // Undeclared prefix: matched as written (no element is named `z:v`).
        assert_eq!(q(x, "//z:v"), Some(vec![]));
        assert_eq!(q(x, "namespace-uri(//b:v)"), None);
        assert_eq!(q(x, "//*[namespace-uri()='urn:y']").unwrap(), ["3"]);
        // Default namespace and namespaced attributes.
        let d = "<feed xmlns=\"urn:atom\" xmlns:g=\"urn:g\"><e g:id=\"7\" id=\"8\">t</e></feed>";
        assert_eq!(q(d, "//e").unwrap(), ["t"]);
        assert_eq!(q(d, "//e/@g:id").unwrap(), ["7"]);
        assert_eq!(q(d, "//*[namespace-uri()='urn:atom']/@id").unwrap(), ["8"]);
        // A redeclared prefix applies only inside its element.
        let s = "<r xmlns:p=\"urn:1\"><p:a>1</p:a><s xmlns:p=\"urn:2\"><p:a>2</p:a></s><p:a>3</p:a></r>";
        assert_eq!(q(s, "//p:a").unwrap(), ["1", "3"]);
    }

    #[test]
    fn lexer_disambiguates() {
        assert_eq!(
            q("<r><n>2</n><n>3</n></r>", "//n[. * 2 = 6]").unwrap(),
            ["3"]
        );
        assert_eq!(
            q("<r><n>2</n><n>3</n></r>", "//n[. mod 2 = 0]").unwrap(),
            ["2"]
        );
        assert_eq!(q("<r><div>1</div></r>", "/r/div").unwrap(), ["1"]);
    }
}
