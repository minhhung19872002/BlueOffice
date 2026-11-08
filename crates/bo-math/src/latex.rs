//! LaTeX math (the common subset of the LaTeX / amsmath documentation) → [`Node`] tree.
//!
//! * groups `{…}`, scripts `^ _` (a group or one token), primes `'`;
//! * `\frac \dfrac \tfrac {a}{b}`, `\binom{n}{k}`, `\sqrt[n]{x}`;
//! * big operators `\sum \prod \coprod \int \iint \iiint \oint \bigcup \bigcap` (`\limits` / `\nolimits`
//!   accepted) with their limits and the operand that follows;
//! * functions `\sin \cos \log \ln \exp \lim \max \min …` (`\lim_{x\to0}` puts the limit below),
//!   `\operatorname{name}`;
//! * `\left( … \right)` with any delimiter (`.` = none), `\{ \}`, `\langle \rangle`, `|`, `\|`;
//! * environments `matrix pmatrix bmatrix Bmatrix vmatrix Vmatrix cases aligned` (`&`, `\\`);
//! * accents `\hat \bar \vec \dot \ddot \tilde \widehat \widetilde`, `\overline \underline`;
//! * `\text{…} \mathrm{…} \textrm{…}` (ordinary text), Greek letters and the usual symbols;
//! * spacing commands (`\, \; \: \! \quad \qquad ~`) are dropped (the layout spaces operators itself).

use crate::unicode_math::{merge, CONTROL_WORDS, FUNCTIONS, LIMIT_FUNCTIONS};
use crate::{Node, Style};

/// Parses LaTeX math into a tree.
#[must_use]
pub fn parse(src: &str) -> Vec<Node> {
    let mut p = Parser {
        t: tokenize(src),
        i: 0,
    };
    let mut out = Vec::new();
    while p.i < p.t.len() {
        let before = p.i;
        out.extend(p.seq());
        if p.i == before {
            // A stray closing token: keep it as text.
            if let Some(Tok::Char(c)) = p.t.get(p.i) {
                out.push(Node::Run(c.to_string(), Style::Math));
            }
            p.i += 1;
        }
    }
    merge(out)
}

/// LaTeX → `<m:oMath>…</m:oMath>`.
#[must_use]
pub fn to_omml(src: &str) -> String {
    crate::to_omml(&parse(src))
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    /// `\name` or a one-character command such as `\{`.
    Cmd(String),
    Char(char),
    Open,
    Close,
    /// The braced argument of a text command, as written (spaces kept).
    Raw(String),
}

/// Commands whose argument is text (its spaces matter).
const TEXT_COMMANDS: &[&str] = &[
    "text",
    "textrm",
    "mathrm",
    "textup",
    "mbox",
    "operatorname",
    "mathit",
    "mathbf",
    "textbf",
    "textit",
    "begin",
    "end",
];

fn tokenize(src: &str) -> Vec<Tok> {
    let c: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        match c[i] {
            '\\' => {
                let mut j = i + 1;
                if j < c.len() && c[j].is_ascii_alphabetic() {
                    while j < c.len() && c[j].is_ascii_alphabetic() {
                        j += 1;
                    }
                } else if j < c.len() {
                    j += 1;
                }
                let name: String = c[i + 1..j].iter().collect();
                i = j;
                if TEXT_COMMANDS.contains(&name.as_str()) && c.get(i) == Some(&'{') {
                    let mut depth = 0;
                    let mut raw = String::new();
                    while i < c.len() {
                        match c[i] {
                            '{' => {
                                depth += 1;
                                if depth > 1 {
                                    raw.push('{');
                                }
                            }
                            '}' => {
                                depth -= 1;
                                if depth == 0 {
                                    i += 1;
                                    break;
                                }
                                raw.push('}');
                            }
                            ch => raw.push(ch),
                        }
                        i += 1;
                    }
                    out.push(Tok::Cmd(name));
                    out.push(Tok::Raw(raw));
                    continue;
                }
                out.push(Tok::Cmd(name));
                continue;
            }
            '{' => out.push(Tok::Open),
            '}' => out.push(Tok::Close),
            '%' => {
                // Comment to the end of the line.
                while i < c.len() && c[i] != '\n' {
                    i += 1;
                }
            }
            ch if ch.is_whitespace() || ch == '~' => {}
            ch => out.push(Tok::Char(ch)),
        }
        i += 1;
    }
    out
}

/// Commands with a LaTeX name different from the UnicodeMath control word, or missing there.
const SYMBOLS: &[(&str, &str)] = &[
    ("neq", "≠"),
    ("lt", "<"),
    ("gt", ">"),
    ("ast", "∗"),
    ("star", "⋆"),
    ("circ", "∘"),
    ("bullet", "∙"),
    ("prime", "′"),
    ("hbar", "ℏ"),
    ("ell", "ℓ"),
    ("Re", "ℜ"),
    ("Im", "ℑ"),
    ("aleph", "ℵ"),
    ("leftrightarrow", "↔"),
    ("Leftarrow", "⇐"),
    ("mapsto", "↦"),
    ("iff", "⟺"),
    ("implies", "⟹"),
    ("supset", "⊃"),
    ("supseteq", "⊇"),
    ("ni", "∋"),
    ("land", "∧"),
    ("wedge", "∧"),
    ("lor", "∨"),
    ("vee", "∨"),
    ("neg", "¬"),
    ("lnot", "¬"),
    ("sim", "∼"),
    ("simeq", "≃"),
    ("cong", "≅"),
    ("propto", "∝"),
    ("parallel", "∥"),
    ("perp", "⊥"),
    ("vdots", "⋮"),
    ("ddots", "⋱"),
    ("dots", "…"),
    ("lbrace", "{"),
    ("rbrace", "}"),
    ("vert", "|"),
    ("Vert", "‖"),
    ("lfloor", "⌊"),
    ("rfloor", "⌋"),
    ("lceil", "⌈"),
    ("rceil", "⌉"),
    ("setminus", "∖"),
    ("bigoplus", "⨁"),
    ("bigotimes", "⨂"),
    ("oplus", "⊕"),
    ("otimes", "⊗"),
    ("varnothing", "∅"),
    ("vartheta", "ϑ"),
    ("varrho", "ϱ"),
    ("varsigma", "ς"),
    ("triangle", "△"),
    ("square", "□"),
    ("%", "%"),
    ("$", "$"),
    ("#", "#"),
    ("_", "_"),
    ("&", "&"),
    ("{", "{"),
    ("}", "}"),
    ("|", "‖"),
];

/// Spacing commands (dropped).
const SPACES: &[&str] = &[
    ",",
    ";",
    ":",
    "!",
    " ",
    "quad",
    "qquad",
    "enspace",
    "thinspace",
];

/// Accent commands → combining character (`None`: a bar).
const ACCENTS: &[(&str, Option<char>)] = &[
    ("hat", Some('\u{302}')),
    ("widehat", Some('\u{302}')),
    ("tilde", Some('\u{303}')),
    ("widetilde", Some('\u{303}')),
    ("vec", Some('\u{20D7}')),
    ("dot", Some('\u{307}')),
    ("ddot", Some('\u{308}')),
    ("check", Some('\u{30C}')),
    ("acute", Some('\u{301}')),
    ("grave", Some('\u{300}')),
    ("breve", Some('\u{306}')),
    ("bar", None),
    ("overline", None),
];

fn symbol(name: &str) -> Option<&'static str> {
    SYMBOLS
        .iter()
        .chain(CONTROL_WORDS.iter())
        .find(|(n, _)| *n == name)
        .map(|(_, c)| *c)
}

struct Parser {
    t: Vec<Tok>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }

    fn is_cmd(&self, name: &str) -> bool {
        matches!(self.peek(), Some(Tok::Cmd(n)) if n == name)
    }

    /// Items up to `}`, `&`, `\\`, `\right` or `\end` (not consumed).
    fn seq(&mut self) -> Vec<Node> {
        let mut out = Vec::new();
        loop {
            match self.peek() {
                None | Some(Tok::Close | Tok::Char('&')) => break,
                Some(Tok::Cmd(n)) if n == "\\" || n == "right" || n == "end" => break,
                _ => {}
            }
            let before = self.i;
            out.extend(self.scripted());
            if self.i == before {
                break;
            }
        }
        merge(out)
    }

    /// An atom with its scripts.
    fn scripted(&mut self) -> Vec<Node> {
        // Big operators take the scripts as limits and the following atom as operand.
        if let Some(Tok::Cmd(n)) = self.peek() {
            if let Some(chr) = nary(n) {
                self.i += 1;
                while self.is_cmd("limits") || self.is_cmd("nolimits") {
                    self.i += 1;
                }
                let (sub, sup) = self.scripts();
                let body = match self.peek() {
                    None | Some(Tok::Close | Tok::Char('&' | '=' | '+' | '-' | '<' | '>')) => {
                        Vec::new()
                    }
                    Some(Tok::Cmd(n)) if n == "\\" || n == "right" || n == "end" => Vec::new(),
                    _ => self.scripted(),
                };
                return vec![Node::Nary {
                    chr,
                    sub,
                    sup,
                    body: merge(body),
                }];
            }
        }
        let base = self.atom();
        let mut primes = String::new();
        while matches!(self.peek(), Some(Tok::Char('\''))) {
            self.i += 1;
            primes.push('′');
        }
        let (sub, sup) = self.scripts();
        let sup = match (primes.is_empty(), sup) {
            (true, s) => s,
            (false, None) => Some(vec![Node::Run(primes, Style::Math)]),
            (false, Some(mut s)) => {
                s.insert(0, Node::Run(primes, Style::Math));
                Some(merge(s))
            }
        };
        if sub.is_none() && sup.is_none() {
            return base;
        }
        // A function name with a script: lim_{x\to0} is a limit, \log_2 a subscripted name.
        if let [Node::Func { name, arg }] = base.as_slice() {
            if arg.is_empty() {
                return vec![self.function_with(name.clone(), sub, sup)];
            }
        }
        vec![Node::Scripts { base, sub, sup }]
    }

    fn scripts(&mut self) -> (Option<Vec<Node>>, Option<Vec<Node>>) {
        let (mut sub, mut sup) = (None, None);
        loop {
            match self.peek() {
                Some(Tok::Char('_')) if sub.is_none() => {
                    self.i += 1;
                    sub = Some(self.argument());
                }
                Some(Tok::Char('^')) if sup.is_none() => {
                    self.i += 1;
                    sup = Some(self.argument());
                }
                _ => break,
            }
        }
        (sub, sup)
    }

    /// A braced group or a single atom (the argument of a command or script).
    fn argument(&mut self) -> Vec<Node> {
        if matches!(self.peek(), Some(Tok::Open)) {
            self.i += 1;
            let v = self.seq();
            if matches!(self.peek(), Some(Tok::Close)) {
                self.i += 1;
            }
            return v;
        }
        // A single digit or letter (x^10 is x¹0 in LaTeX).
        if let Some(Tok::Char(c)) = self.peek() {
            let c = *c;
            self.i += 1;
            return vec![Node::Run(c.to_string(), Style::Math)];
        }
        self.atom()
    }

    /// `name` with sub/superscripts and the following argument (functions).
    fn function_with(
        &mut self,
        name: Vec<Node>,
        sub: Option<Vec<Node>>,
        sup: Option<Vec<Node>>,
    ) -> Node {
        let is_limit =
            matches!(name.as_slice(), [Node::Run(n, _)] if LIMIT_FUNCTIONS.contains(&n.as_str()));
        let mut head = name;
        let mut sub = sub;
        if is_limit {
            if let Some(lim) = sub.take() {
                head = vec![Node::Limit {
                    base: head,
                    lim,
                    upper: false,
                }];
            }
        }
        if sub.is_some() || sup.is_some() {
            head = vec![Node::Scripts {
                base: head,
                sub,
                sup,
            }];
        }
        let arg = self.function_arg();
        Node::Func { name: head, arg }
    }

    fn function_arg(&mut self) -> Vec<Node> {
        match self.peek() {
            None | Some(Tok::Close | Tok::Char('&' | '=' | '+' | '-' | '<' | '>' | ',')) => {
                Vec::new()
            }
            Some(Tok::Cmd(n)) if n == "\\" || n == "right" || n == "end" => Vec::new(),
            _ => merge(self.scripted()),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn atom(&mut self) -> Vec<Node> {
        let Some(tok) = self.peek().cloned() else {
            return Vec::new();
        };
        self.i += 1;
        match tok {
            Tok::Open => {
                let v = self.seq();
                if matches!(self.peek(), Some(Tok::Close)) {
                    self.i += 1;
                }
                v
            }
            Tok::Close => {
                self.i -= 1;
                Vec::new()
            }
            Tok::Char(c) if c.is_ascii_digit() => {
                let mut t = c.to_string();
                while let Some(Tok::Char(d)) = self.peek() {
                    let decimal = *d == '.'
                        && matches!(self.t.get(self.i + 1), Some(Tok::Char(n)) if n.is_ascii_digit());
                    if d.is_ascii_digit() || decimal {
                        t.push(*d);
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                vec![Node::Run(t, Style::Math)]
            }
            Tok::Char('-') => vec![Node::Run("−".into(), Style::Math)],
            // (x), [x], |x|: brackets that pair up at the same group level grow around their
            // content (as typed in an equation editor; \left … \right does the same explicitly).
            Tok::Char(c @ ('(' | '[' | '|')) => {
                let close = match c {
                    '(' => ')',
                    '[' => ']',
                    _ => '|',
                };
                match self.matching(c, close) {
                    Some(j) => {
                        let mut inner = Parser {
                            t: self.t[self.i..j].to_vec(),
                            i: 0,
                        };
                        let content = inner.seq();
                        self.i = j + 1;
                        vec![Node::Delim {
                            open: Some(c),
                            close: Some(close),
                            parts: vec![content],
                        }]
                    }
                    None => vec![Node::Run(c.to_string(), Style::Math)],
                }
            }
            Tok::Char(c) => vec![Node::Run(c.to_string(), Style::Math)],
            Tok::Cmd(name) => self.command(&name),
            Tok::Raw(r) => vec![Node::Run(r, Style::Normal)],
        }
    }

    #[allow(clippy::too_many_lines)]
    fn command(&mut self, name: &str) -> Vec<Node> {
        match name {
            "frac" | "dfrac" | "tfrac" | "cfrac" => {
                let num = self.argument();
                let den = self.argument();
                vec![Node::Frac {
                    num,
                    den,
                    bar: true,
                }]
            }
            "binom" | "dbinom" | "tbinom" => {
                let num = self.argument();
                let den = self.argument();
                vec![Node::Delim {
                    open: Some('('),
                    close: Some(')'),
                    parts: vec![vec![Node::Frac {
                        num,
                        den,
                        bar: false,
                    }]],
                }]
            }
            "sqrt" => {
                let deg = if matches!(self.peek(), Some(Tok::Char('['))) {
                    self.i += 1;
                    let mut d = Vec::new();
                    while let Some(t) = self.peek() {
                        if matches!(t, Tok::Char(']')) {
                            self.i += 1;
                            break;
                        }
                        let before = self.i;
                        d.extend(self.scripted());
                        if self.i == before {
                            self.i += 1;
                        }
                    }
                    Some(merge(d))
                } else {
                    None
                };
                let body = self.argument();
                vec![Node::Rad { deg, body }]
            }
            "left" => {
                let open = self.delimiter();
                let inner = self.seq();
                let close = if self.is_cmd("right") {
                    self.i += 1;
                    self.delimiter()
                } else {
                    None
                };
                vec![Node::Delim {
                    open,
                    close,
                    parts: vec![inner],
                }]
            }
            "begin" => self.environment(),
            "text" | "textrm" | "mathrm" | "textup" | "mbox" | "operatorname" | "mathit"
            | "mathbf" | "textbf" | "textit" => {
                let style = match name {
                    "operatorname" | "mathrm" => Style::Upright,
                    "mathit" => Style::Math,
                    _ => Style::Normal,
                };
                let text = self.raw_group();
                if name == "operatorname" {
                    let arg = self.function_arg();
                    return vec![Node::Func {
                        name: vec![Node::Run(text, Style::Upright)],
                        arg,
                    }];
                }
                vec![Node::Run(text, style)]
            }
            "underline" => {
                let base = self.argument();
                vec![Node::Bar { top: false, base }]
            }
            n if ACCENTS.iter().any(|(a, _)| *a == n) => {
                let chr = ACCENTS.iter().find(|(a, _)| *a == n).and_then(|(_, c)| *c);
                let base = self.argument();
                vec![match chr {
                    Some(chr) => Node::Accent { chr, base },
                    None => Node::Bar { top: true, base },
                }]
            }
            n if FUNCTIONS.contains(&n) => {
                // Scripts are handled by `scripted` (it rebuilds the function with its limit).
                if matches!(self.peek(), Some(Tok::Char('_' | '^'))) {
                    return vec![Node::Func {
                        name: vec![Node::Run(n.to_owned(), Style::Upright)],
                        arg: Vec::new(),
                    }];
                }
                let arg = self.function_arg();
                vec![Node::Func {
                    name: vec![Node::Run(n.to_owned(), Style::Upright)],
                    arg,
                }]
            }
            n if SPACES.contains(&n) => Vec::new(),
            "displaystyle" | "textstyle" | "limits" | "nolimits" | "big" | "Big" | "bigg"
            | "Bigg" | "bigl" | "bigr" | "Bigl" | "Bigr" => Vec::new(),
            n => match symbol(n) {
                Some(s) => vec![Node::Run(s.to_owned(), Style::Math)],
                // Unknown command: keep its name as text so nothing typed is lost.
                None => vec![Node::Run(format!("\\{n}"), Style::Normal)],
            },
        }
    }

    /// Index of the `close` token matching an `open` just consumed, at the same group level.
    fn matching(&self, open: char, close: char) -> Option<usize> {
        let (mut depth, mut nest) = (0i32, 0i32);
        let mut j = self.i;
        while let Some(t) = self.t.get(j) {
            match t {
                Tok::Open => depth += 1,
                Tok::Close | Tok::Char('&') if depth == 0 => return None,
                Tok::Close => depth -= 1,
                Tok::Cmd(n) if depth == 0 && (n == "\\" || n == "right" || n == "end") => {
                    return None
                }
                Tok::Char(ch) if depth == 0 && *ch == close && nest == 0 => return Some(j),
                Tok::Char(ch) if depth == 0 && *ch == close => nest -= 1,
                Tok::Char(ch) if depth == 0 && *ch == open && open != close => nest += 1,
                _ => {}
            }
            j += 1;
        }
        None
    }

    /// The delimiter after `\left` / `\right` (`.` = none).
    fn delimiter(&mut self) -> Option<char> {
        let t = self.peek().cloned()?;
        self.i += 1;
        match t {
            Tok::Char(c) if c != '.' => Some(c),
            Tok::Cmd(n) => symbol(&n).and_then(|s| s.chars().next()),
            Tok::Char(_) | Tok::Open | Tok::Close | Tok::Raw(_) => None,
        }
    }

    /// The characters of a braced group, as written (for `\text`).
    fn raw_group(&mut self) -> String {
        if let Some(Tok::Raw(r)) = self.peek().cloned() {
            self.i += 1;
            // Commands inside text: spacing and symbols.
            let mut out = String::new();
            let cs: Vec<char> = r.chars().collect();
            let mut k = 0;
            while k < cs.len() {
                if cs[k] == '\\' {
                    let mut j = k + 1;
                    while j < cs.len() && cs[j].is_ascii_alphabetic() {
                        j += 1;
                    }
                    if j == k + 1 && j < cs.len() {
                        j += 1;
                    }
                    let n: String = cs[k + 1..j].iter().collect();
                    if SPACES.contains(&n.as_str()) {
                        out.push(' ');
                    } else if let Some(sym) = symbol(&n) {
                        out.push_str(sym);
                    }
                    k = j;
                    continue;
                }
                out.push(cs[k]);
                k += 1;
            }
            return out;
        }
        let mut s = String::new();
        if !matches!(self.peek(), Some(Tok::Open)) {
            return s;
        }
        self.i += 1;
        let mut depth = 1;
        while let Some(t) = self.peek().cloned() {
            self.i += 1;
            match t {
                Tok::Open => depth += 1,
                Tok::Close => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                Tok::Char(c) => s.push(c),
                Tok::Cmd(n) => {
                    if SPACES.contains(&n.as_str()) {
                        s.push(' ');
                    } else if let Some(sym) = symbol(&n) {
                        s.push_str(sym);
                    }
                }
                Tok::Raw(r) => s.push_str(&r),
            }
        }
        s
    }

    /// `\begin{env} … \end{env}`: matrices, cases and aligned equations.
    fn environment(&mut self) -> Vec<Node> {
        let env = self.raw_group();
        let mut rows: Vec<Vec<Vec<Node>>> = vec![vec![]];
        loop {
            let cell = self.seq();
            if let Some(r) = rows.last_mut() {
                r.push(cell);
            }
            match self.peek() {
                Some(Tok::Cmd(n)) if n == "\\" => {
                    self.i += 1;
                    rows.push(Vec::new());
                }
                Some(Tok::Cmd(n)) if n == "end" => {
                    self.i += 1;
                    let _ = self.raw_group();
                    break;
                }
                None => break,
                // `&` between cells, a stray `}`, anything else: step over it.
                _ => self.i += 1,
            }
        }
        // A trailing `\\` leaves an empty last row.
        if rows.len() > 1 && rows.last().is_some_and(|r| r.iter().all(Vec::is_empty)) {
            rows.pop();
        }
        let delims = match env.as_str() {
            "pmatrix" => Some((Some('('), Some(')'))),
            "bmatrix" => Some((Some('['), Some(']'))),
            "Bmatrix" => Some((Some('{'), Some('}'))),
            "vmatrix" => Some((Some('|'), Some('|'))),
            "Vmatrix" => Some((Some('‖'), Some('‖'))),
            _ => None,
        };
        match env.as_str() {
            // Value and condition of each case, an em space apart.
            "cases" => vec![Node::Delim {
                open: Some('{'),
                close: None,
                parts: vec![vec![Node::EqArray(
                    rows.into_iter()
                        .map(|r| {
                            let mut row = Vec::new();
                            for (k, cell) in r.into_iter().enumerate() {
                                if k > 0 {
                                    row.push(Node::Run("\u{2003}".into(), Style::Math));
                                }
                                row.extend(cell);
                            }
                            merge(row)
                        })
                        .collect(),
                )]],
            }],
            "aligned" | "align" | "align*" | "gathered" | "split" | "array" | "eqnarray" => {
                vec![Node::EqArray(
                    rows.into_iter()
                        .map(|r| merge(r.into_iter().flatten().collect()))
                        .collect(),
                )]
            }
            _ => {
                let m = Node::Matrix(rows);
                match delims {
                    Some((open, close)) => vec![Node::Delim {
                        open,
                        close,
                        parts: vec![vec![m]],
                    }],
                    None => vec![m],
                }
            }
        }
    }
}

fn nary(name: &str) -> Option<char> {
    Some(match name {
        "sum" => '∑',
        "prod" => '∏',
        "coprod" => '∐',
        "int" => '∫',
        "iint" => '∬',
        "iiint" => '∭',
        "oint" => '∮',
        "bigcup" => '⋃',
        "bigcap" => '⋂',
        "bigvee" => '⋁',
        "bigwedge" => '⋀',
        "bigoplus" => '⨁',
        "bigotimes" => '⨂',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same equations in LaTeX and in UnicodeMath give the same OMML.
    // source: amsmath / LaTeX math documentation examples and their UnicodeMath (UTN #28) spellings.
    #[test]
    fn latex_matches_unicode_math() {
        let pairs = [
            (r"\frac{a+b}{c}", "(a+b)/c"),
            (r"x^2+y^2=z^2", "x^2+y^2=z^2"),
            (r"x_{i}^{2}", "x_i^2"),
            (r"e^{-x}", "e^(-x)"),
            (r"\sqrt{x+1}", "√(x+1)"),
            (r"\sqrt[3]{x}", "√(3&x)"),
            (r"\sum_{k=1}^{n} k", "∑_(k=1)^n k"),
            (r"\int_0^1 x\,dx", "∫_0^1 x dx"),
            (r"\left( \frac{a}{b} \right)", "(a/b)"),
            (r"\left[ x \right]", "[x]"),
            (r"|x|", "|x|"),
            (r"\sin x", "sin x"),
            (r"\lim_{x \to 0} x", "lim_(x→0) x"),
            (r"\log_2 x", "log_2 x"),
            (r"\begin{matrix} 1 & 0 \\ 0 & 1 \end{matrix}", "■(1&0@0&1)"),
            (
                r"\begin{bmatrix} a & b \\ c & d \end{bmatrix}",
                "[■(a&b@c&d)]",
            ),
            (r"\hat{x}+\vec{v}", "x\u{302}+v\u{20D7}"),
            (r"\overline{y}", "y\u{305}"),
            (r"\alpha+\beta\le\infty", "α+β≤∞"),
            (r"\text{if } x>0", "\"if \" x>0"),
            (r"f'(x)", "f'(x)"),
            (r"a \cdot b \times c \pm d", "a⋅b×c±d"),
        ];
        for (tex, um) in pairs {
            assert_eq!(to_omml(tex), crate::unicode_math::to_omml(um), "{tex}");
        }
    }

    // source: ECMA-376 Part 1 §22.1.2 shapes for amsmath constructs without a UnicodeMath twin above.
    #[test]
    fn latex_specific_constructs() {
        let x = to_omml(r"\binom{n}{k}");
        assert!(
            x.contains(r#"<m:type m:val="noBar"/>"#) && x.starts_with("<m:oMath><m:d>"),
            "{x}"
        );
        let x = to_omml(r"f(x)=\begin{cases} 1 & x>0 \\ 0 & x\le 0 \end{cases}");
        assert!(
            x.contains(r#"<m:begChr m:val="{"/><m:endChr m:val=""/>"#),
            "{x}"
        );
        assert!(
            x.contains("<m:eqArr><m:e><m:r><m:t>1\u{2003}x&gt;0</m:t></m:r></m:e><m:e><m:r><m:t>0\u{2003}x≤0</m:t></m:r></m:e></m:eqArr>"),
            "{x}"
        );
        let x = to_omml(r"\left. \frac{df}{dx} \right|_{x=0}");
        assert!(
            x.contains(r#"<m:begChr m:val=""/><m:endChr m:val="|"/>"#),
            "{x}"
        );
        assert!(x.contains("<m:sSub>"), "{x}");
        let x = to_omml(r"\operatorname{sgn} x");
        assert!(
            x.contains(r#"<m:sty m:val="p"/></m:rPr><m:t>sgn</m:t>"#),
            "{x}"
        );
        // Unknown commands are kept visibly.
        assert!(to_omml(r"\foo x").contains(r"\foo"));
        // Comments and spacing commands are dropped.
        assert_eq!(
            to_omml("a\\,b % note\n+c"),
            crate::unicode_math::to_omml("ab+c")
        );
    }

    proptest::proptest! {
        #[test]
        fn never_panics(s in "[\\\\a-z{}^_&\\[\\]()0-9 +=-]{0,40}") {
            let x = to_omml(&s);
            proptest::prop_assert!(x.starts_with("<m:oMath>") && x.ends_with("</m:oMath>"));
            proptest::prop_assert_eq!(x.matches("<m:e>").count(), x.matches("</m:e>").count());
        }
    }
}
