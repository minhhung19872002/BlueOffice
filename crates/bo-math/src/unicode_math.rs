//! UnicodeMath linear format → [`Node`] tree, after the public Unicode Technical Note #28 ("UnicodeMath, a
//! Nearly Plain-Text Encoding of Mathematics", M. Sargent): the subset people type in an equation box.
//!
//! * operands: letters (one per italic run), numbers (`3.14`), Greek and other symbols, `"literal text"`;
//! * `a/b` fraction (the operand before and after; parentheses around an operand are removed), `a¦b`
//!   stacked without a bar;
//! * `x_i`, `x^2`, `x_i^2`, primes `f'`;
//! * `√x`, `√(x+1)`, `∛x`, `∜x`, `√(n&x)` (degree `n`);
//! * n-ary operators `∑ ∏ ∐ ∫ ∬ ∭ ∮ ⋃ ⋂ …` with `_` / `^` limits and the operand that follows
//!   (optionally after `▒`);
//! * brackets `( ) [ ] { } ⟨ ⟩ ⌊ ⌋ ⌈ ⌉ | |` grow around their content, `∣` separates parts;
//! * functions `sin cos tan log ln exp lim max min …` (upright, `lim_(x→0)` puts the limit below);
//! * `■(a&b@c&d)` matrix, `█(a@b)` equation array;
//! * combining accents after a letter (`x̂`, `v⃗`), `x̅` overbar;
//! * control words `\alpha`, `\sum`, `\sqrt`, `\infty`, `\to`, `\le`, `\hat` … for keyboard input.
//!
//! Spaces separate operands and are not kept; `-` becomes the minus sign `−` (U+2212).

use crate::{Node, Pr, Style};

/// Parses UnicodeMath into a tree.
#[must_use]
pub fn parse(src: &str) -> Vec<Node> {
    let mut p = Parser {
        c: expand_control_words(src).chars().collect(),
        i: 0,
    };
    let mut out = p.seq(&[]);
    // Stray closing characters at the top level are kept as text.
    while p.i < p.c.len() {
        out.push(Node::run(p.c[p.i].to_string(), Style::Math));
        p.i += 1;
        out.extend(p.seq(&[]));
    }
    merge(out)
}

/// UnicodeMath → `<m:oMath>…</m:oMath>`.
#[must_use]
pub fn to_omml(src: &str) -> String {
    crate::to_omml(&parse(src))
}

pub(crate) const CONTROL_WORDS: &[(&str, &str)] = &[
    ("alpha", "α"),
    ("beta", "β"),
    ("gamma", "γ"),
    ("delta", "δ"),
    ("epsilon", "ϵ"),
    ("varepsilon", "ε"),
    ("zeta", "ζ"),
    ("eta", "η"),
    ("theta", "θ"),
    ("iota", "ι"),
    ("kappa", "κ"),
    ("lambda", "λ"),
    ("mu", "μ"),
    ("nu", "ν"),
    ("xi", "ξ"),
    ("pi", "π"),
    ("rho", "ρ"),
    ("sigma", "σ"),
    ("tau", "τ"),
    ("upsilon", "υ"),
    ("phi", "ϕ"),
    ("varphi", "φ"),
    ("chi", "χ"),
    ("psi", "ψ"),
    ("omega", "ω"),
    ("Gamma", "Γ"),
    ("Delta", "Δ"),
    ("Theta", "Θ"),
    ("Lambda", "Λ"),
    ("Xi", "Ξ"),
    ("Pi", "Π"),
    ("Sigma", "Σ"),
    ("Phi", "Φ"),
    ("Psi", "Ψ"),
    ("Omega", "Ω"),
    ("infty", "∞"),
    ("partial", "∂"),
    ("nabla", "∇"),
    ("pm", "±"),
    ("mp", "∓"),
    ("times", "×"),
    ("div", "÷"),
    ("cdot", "⋅"),
    ("le", "≤"),
    ("leq", "≤"),
    ("ge", "≥"),
    ("geq", "≥"),
    ("ne", "≠"),
    ("neq", "≠"),
    ("approx", "≈"),
    ("equiv", "≡"),
    ("to", "→"),
    ("rightarrow", "→"),
    ("leftarrow", "←"),
    ("Rightarrow", "⇒"),
    ("Leftrightarrow", "⇔"),
    ("in", "∈"),
    ("notin", "∉"),
    ("subset", "⊂"),
    ("subseteq", "⊆"),
    ("cup", "∪"),
    ("cap", "∩"),
    ("forall", "∀"),
    ("exists", "∃"),
    ("emptyset", "∅"),
    ("angle", "∠"),
    ("degree", "°"),
    ("cdots", "⋯"),
    ("ldots", "…"),
    ("sum", "∑"),
    ("prod", "∏"),
    ("coprod", "∐"),
    ("int", "∫"),
    ("iint", "∬"),
    ("iiint", "∭"),
    ("oint", "∮"),
    ("bigcup", "⋃"),
    ("bigcap", "⋂"),
    ("sqrt", "√"),
    ("cbrt", "∛"),
    ("qdrt", "∜"),
    ("matrix", "■"),
    ("eqarray", "█"),
    ("naryand", "▒"),
    ("of", "▒"),
    ("atop", "¦"),
    ("mid", "∣"),
    ("langle", "⟨"),
    ("rangle", "⟩"),
    ("hat", "\u{302}"),
    ("tilde", "\u{303}"),
    ("bar", "\u{305}"),
    ("dot", "\u{307}"),
    ("ddot", "\u{308}"),
    ("vec", "\u{20D7}"),
];

/// Replaces `\word` by its character (a space right after the word is consumed).
pub(crate) fn expand_control_words(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' {
            let mut j = i + 1;
            while j < chars.len() && chars[j].is_ascii_alphabetic() {
                j += 1;
            }
            let word: String = chars[i + 1..j].iter().collect();
            if let Some((_, c)) = CONTROL_WORDS.iter().find(|(w, _)| *w == word) {
                out.push_str(c);
                i = j;
                if chars.get(i) == Some(&' ') {
                    i += 1;
                }
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// Function names written upright (§22.1.2.39 func).
pub(crate) const FUNCTIONS: &[&str] = &[
    "arccos", "arcsin", "arctan", "arg", "cos", "cosh", "cot", "coth", "csc", "deg", "det", "dim",
    "exp", "gcd", "inf", "ker", "lg", "lim", "ln", "log", "max", "min", "mod", "sec", "sin",
    "sinh", "sup", "tan", "tanh",
];

/// Functions whose `_` script is a limit below the name (`lim_(x→0)`).
pub(crate) const LIMIT_FUNCTIONS: &[&str] = &["lim", "max", "min", "sup", "inf"];

pub(crate) const NARY: &str = "∑∏∐∫∬∭∮∯∰⋃⋂⋁⋀⨁⨂⨀";

/// Binary operators, relations and punctuation (kept as text between operands).
const OPERATORS: &str = "+-−=<>≤≥≠±∓×÷⋅·∙→←↔⇒⇐⇔∈∉∋⊂⊃⊆⊇∪∩∧∨¬≈≡≅∼∝∥⊥,;:!∀∃";

fn opening(c: char) -> Option<char> {
    Some(match c {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        '⟨' => '⟩',
        '⌊' => '⌋',
        '⌈' => '⌉',
        '‖' => '‖',
        _ => return None,
    })
}

fn is_closing(c: char) -> bool {
    ")]}⟩⌋⌉".contains(c)
}

fn is_combining(c: char) -> bool {
    matches!(c as u32, 0x300..=0x36F | 0x20D0..=0x20FF)
}

struct Parser {
    c: Vec<char>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }

    fn skip_spaces(&mut self) {
        while self.peek().is_some_and(|c| c == ' ' || c == '\u{2061}') {
            self.i += 1;
        }
    }

    /// Operands and operators up to a character of `stop` (not consumed) or the end.
    fn seq(&mut self, stop: &[char]) -> Vec<Node> {
        let mut out = Vec::new();
        loop {
            self.skip_spaces();
            let Some(c) = self.peek() else { break };
            if stop.contains(&c) || is_closing(c) && !stop.is_empty() {
                break;
            }
            if is_closing(c) {
                break;
            }
            if OPERATORS.contains(c) {
                self.i += 1;
                let op = if c == '-' { '−' } else { c };
                out.push(Node::run(op.to_string(), Style::Math));
                continue;
            }
            if matches!(c, '/' | '¦' | '_' | '^') {
                // An operator with no operand before it: keep the character.
                self.i += 1;
                out.push(Node::run(c.to_string(), Style::Math));
                continue;
            }
            let before = self.i;
            out.extend(self.term(stop));
            if self.i == before {
                // Nothing parsed (a stop inside a term): avoid looping.
                break;
            }
        }
        out
    }

    /// Factors written together, then fractions (`/` binds the operands on each side).
    fn term(&mut self, stop: &[char]) -> Vec<Node> {
        let mut num = self.factors(stop);
        while let Some(c @ ('/' | '¦')) = self.peek() {
            self.i += 1;
            let den = self.factors(stop);
            num = vec![Node::Frac {
                pr: Pr::default(),
                num: strip(num),
                den: strip(den),
                bar: c == '/',
            }];
        }
        num
    }

    fn factors(&mut self, stop: &[char]) -> Vec<Node> {
        let mut out = Vec::new();
        while let Some(c) = self.peek() {
            if c == ' '
                || OPERATORS.contains(c)
                || stop.contains(&c)
                || is_closing(c)
                || matches!(c, '/' | '¦' | '_' | '^' | '&' | '@' | '∣' | '▒')
                || (c == '|' && stop.contains(&'|'))
            {
                break;
            }
            let before = self.i;
            out.extend(self.factor(stop));
            if self.i == before {
                break;
            }
        }
        out
    }

    /// One primary with its scripts, primes and accents.
    fn factor(&mut self, stop: &[char]) -> Vec<Node> {
        let mut base = self.primary(stop);
        // Combining accents and the overbar after the base.
        while let Some(c) = self.peek().filter(|c| is_combining(*c)) {
            self.i += 1;
            base = vec![match c {
                '\u{305}' => Node::Bar {
                    pr: Pr::default(),
                    top: true,
                    base: strip(base),
                },
                '\u{332}' => Node::Bar {
                    pr: Pr::default(),
                    top: false,
                    base: strip(base),
                },
                _ => Node::Accent {
                    pr: Pr::default(),
                    chr: c,
                    base: strip(base),
                },
            }];
        }
        // Primes: f′, f″.
        let mut primes = String::new();
        while let Some(c) = self.peek().filter(|c| matches!(c, '\'' | '′')) {
            let _ = c;
            self.i += 1;
            primes.push('′');
        }
        let (mut sub, mut sup) = (None, None);
        if !primes.is_empty() {
            let p = match primes.chars().count() {
                2 => "″".to_owned(),
                3 => "‴".to_owned(),
                _ => primes,
            };
            sup = Some(vec![Node::run(p, Style::Math)]);
        }
        loop {
            match self.peek() {
                Some('_') if sub.is_none() => {
                    self.i += 1;
                    sub = Some(self.script());
                }
                Some('^') if sup.is_none() => {
                    self.i += 1;
                    sup = Some(self.script());
                }
                _ => break,
            }
        }
        if sub.is_none() && sup.is_none() {
            return base;
        }
        vec![Node::Scripts {
            pr: Pr::default(),
            base: strip(base),
            sub,
            sup,
        }]
    }

    /// The operand of `_` / `^`: a bracketed group (brackets removed), a signed operand, or one primary.
    fn script(&mut self) -> Vec<Node> {
        match self.peek() {
            Some(c @ ('-' | '+' | '−' | '±')) => {
                self.i += 1;
                let mut v = vec![Node::run(
                    if c == '-' { '−' } else { c }.to_string(),
                    Style::Math,
                )];
                v.extend(self.primary(&[]));
                merge(v)
            }
            _ => strip(self.primary(&[])),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn primary(&mut self, stop: &[char]) -> Vec<Node> {
        let Some(c) = self.peek() else {
            return Vec::new();
        };
        // Brackets: content grows the delimiters; `∣` separates parts.
        if let Some(close) = opening(c) {
            self.i += 1;
            return vec![self.delimited(Some(c), close)];
        }
        if c == '|' && !stop.contains(&'|') {
            self.i += 1;
            return vec![self.delimited(Some('|'), '|')];
        }
        match c {
            '√' | '∛' | '∜' => {
                self.i += 1;
                self.skip_spaces();
                let deg = match c {
                    '∛' => Some(vec![Node::run("3", Style::Math)]),
                    '∜' => Some(vec![Node::run("4", Style::Math)]),
                    _ => None,
                };
                if c == '√' && self.peek() == Some('(') {
                    // √(n&x): degree n.
                    self.i += 1;
                    let first = self.seq(&[')', '&']);
                    if self.peek() == Some('&') {
                        self.i += 1;
                        let body = self.seq(&[')']);
                        self.eat(')');
                        return vec![Node::Rad {
                            pr: Pr::default(),
                            deg: Some(merge(first)),
                            body: merge(body),
                        }];
                    }
                    self.eat(')');
                    return vec![Node::Rad {
                        pr: Pr::default(),
                        deg: None,
                        body: merge(first),
                    }];
                }
                let body = strip(self.factor(stop));
                vec![Node::Rad {
                    pr: Pr::default(),
                    deg,
                    body: merge(body),
                }]
            }
            c if NARY.contains(c) => {
                self.i += 1;
                let (mut sub, mut sup) = (None, None);
                loop {
                    match self.peek() {
                        Some('_') if sub.is_none() => {
                            self.i += 1;
                            sub = Some(self.script());
                        }
                        Some('^') if sup.is_none() => {
                            self.i += 1;
                            sup = Some(self.script());
                        }
                        _ => break,
                    }
                }
                self.skip_spaces();
                if self.peek() == Some('▒') {
                    self.i += 1;
                    self.skip_spaces();
                }
                let body = if self.peek().is_some_and(|n| {
                    !OPERATORS.contains(n)
                        && !stop.contains(&n)
                        && !is_closing(n)
                        && n != '&'
                        && n != '@'
                }) {
                    strip(self.factors(stop))
                } else {
                    Vec::new()
                };
                vec![Node::Nary {
                    pr: Pr::default(),
                    chr: c,
                    sub,
                    sup,
                    body: merge(body),
                }]
            }
            '■' | '█' => {
                self.i += 1;
                self.skip_spaces();
                if !self.eat('(') {
                    return vec![Node::run(c.to_string(), Style::Math)];
                }
                let mut rows: Vec<Vec<Vec<Node>>> = vec![vec![]];
                loop {
                    let cell = merge(self.seq(&['&', '@', ')']));
                    if let Some(r) = rows.last_mut() {
                        r.push(cell);
                    }
                    match self.peek() {
                        Some('&') => self.i += 1,
                        Some('@') => {
                            self.i += 1;
                            rows.push(Vec::new());
                        }
                        _ => {
                            self.eat(')');
                            break;
                        }
                    }
                }
                if c == '■' {
                    vec![Node::Matrix(rows, Pr::default())]
                } else {
                    // Equation array: `&` marks alignment points, the cells of a row join.
                    vec![Node::EqArray(
                        rows.into_iter()
                            .map(|r| merge(r.into_iter().flatten().collect()))
                            .collect(),
                        Pr::default(),
                    )]
                }
            }
            '"' => {
                self.i += 1;
                let mut t = String::new();
                while let Some(ch) = self.peek() {
                    self.i += 1;
                    if ch == '"' {
                        break;
                    }
                    t.push(ch);
                }
                vec![Node::run(t, Style::Normal)]
            }
            c if c.is_ascii_digit() => {
                let mut t = String::new();
                while let Some(ch) = self.peek() {
                    let decimal = matches!(ch, '.' | ',')
                        && self.c.get(self.i + 1).is_some_and(char::is_ascii_digit)
                        && !t.is_empty();
                    if ch.is_ascii_digit() || decimal && ch == '.' {
                        t.push(ch);
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                vec![Node::run(t, Style::Math)]
            }
            c if c.is_ascii_alphabetic() => {
                let start = self.i;
                let mut j = self.i;
                while self.c.get(j).is_some_and(char::is_ascii_alphabetic) {
                    j += 1;
                }
                let word: String = self.c[start..j].iter().collect();
                // The longest function name at the start of the letters.
                let name = FUNCTIONS
                    .iter()
                    .filter(|f| word.starts_with(**f) && (word.len() == f.len()))
                    .max_by_key(|f| f.len());
                if let Some(name) = name {
                    self.i = start + name.len();
                    return self.function(name, stop);
                }
                self.i += 1;
                vec![Node::run(c.to_string(), Style::Math)]
            }
            _ => {
                self.i += 1;
                vec![Node::run(c.to_string(), Style::Math)]
            }
        }
    }

    /// `name` (already consumed), its scripts or limit, and its argument.
    fn function(&mut self, name: &str, stop: &[char]) -> Vec<Node> {
        let mut head = vec![Node::run(name.to_owned(), Style::Upright)];
        let limit = LIMIT_FUNCTIONS.contains(&name);
        let (mut sub, mut sup) = (None, None);
        loop {
            match self.peek() {
                Some('_') if sub.is_none() => {
                    self.i += 1;
                    sub = Some(self.script());
                }
                Some('^') if sup.is_none() => {
                    self.i += 1;
                    sup = Some(self.script());
                }
                _ => break,
            }
        }
        if limit {
            if let Some(lim) = sub.take() {
                head = vec![Node::Limit {
                    pr: Pr::default(),
                    base: head,
                    lim,
                    upper: false,
                }];
            }
        }
        if sub.is_some() || sup.is_some() {
            head = vec![Node::Scripts {
                pr: Pr::default(),
                base: head,
                sub,
                sup,
            }];
        }
        self.skip_spaces();
        let arg = match self.peek() {
            Some(n)
                if !OPERATORS.contains(n)
                    && !stop.contains(&n)
                    && !is_closing(n)
                    && !matches!(n, '&' | '@' | '/' | '¦') =>
            {
                // lim / max / min take the term that follows (lim_(x→0) sin x/x is the limit of the
                // fraction); other functions take one factor (sin x/x is (sin x)/x).
                if limit {
                    self.term(stop)
                } else {
                    self.factor(stop)
                }
            }
            _ => Vec::new(),
        };
        vec![Node::Func {
            pr: Pr::default(),
            name: head,
            arg: merge(arg),
        }]
    }

    /// Content of brackets opened by `open` up to `close` (consumed if present).
    fn delimited(&mut self, open: Option<char>, close: char) -> Node {
        let mut parts = Vec::new();
        let stops = [close, '∣'];
        loop {
            parts.push(merge(self.seq(&stops)));
            match self.peek() {
                Some('∣') => self.i += 1,
                Some(c) if c == close => {
                    self.i += 1;
                    return Node::Delim {
                        pr: Pr::default(),
                        open,
                        close: Some(close),
                        parts,
                    };
                }
                _ => {
                    return Node::Delim {
                        pr: Pr::default(),
                        open,
                        close: None,
                        parts,
                    }
                }
            }
        }
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.i += 1;
            true
        } else {
            false
        }
    }
}

/// An operand in parentheses loses them when it is the operand of a fraction, script or radical
/// (UTN #28: "the parentheses are removed when the expression is built up").
fn strip(nodes: Vec<Node>) -> Vec<Node> {
    if let [Node::Delim {
        open: Some('('),
        close: Some(')'),
        parts,
        ..
    }] = nodes.as_slice()
    {
        if parts.len() == 1 {
            return parts[0].clone();
        }
    }
    merge(nodes)
}

/// Joins neighbouring runs of the same style and run properties (one `m:r` per stretch of text).
pub(crate) fn merge(nodes: Vec<Node>) -> Vec<Node> {
    let mut out: Vec<Node> = Vec::with_capacity(nodes.len());
    for n in nodes {
        if let (Some(Node::Run(prev, ps, pp)), Node::Run(t, s, p)) = (out.last_mut(), &n) {
            // The formatting compared, not how the modelled values were read.
            if ps == s && *s != Style::Upright && pp.math.kids == p.math.kids && pp.text == p.text {
                prev.push_str(t);
                continue;
            }
        }
        out.push(n);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(t: &str) -> String {
        format!("<m:r><m:t>{t}</m:t></m:r>")
    }

    fn m(inner: &str) -> String {
        format!("<m:oMath>{inner}</m:oMath>")
    }

    // source: examples of Unicode Technical Note #28 (UnicodeMath) — expected OMML written from ECMA-376
    // Part 1, §22.1 element definitions.
    #[test]
    fn linear_format_table() {
        let frac = |n: &str, d: &str| format!("<m:f><m:num>{n}</m:num><m:den>{d}</m:den></m:f>");
        let sup = |e: &str, s: &str| format!("<m:sSup><m:e>{e}</m:e><m:sup>{s}</m:sup></m:sSup>");
        let sub = |e: &str, s: &str| format!("<m:sSub><m:e>{e}</m:e><m:sub>{s}</m:sub></m:sSub>");
        let sqrt = |e: &str| {
            format!(
                r#"<m:rad><m:radPr><m:degHide m:val="1"/></m:radPr><m:deg/><m:e>{e}</m:e></m:rad>"#
            )
        };
        let up = |t: &str| format!(r#"<m:r><m:rPr><m:sty m:val="p"/></m:rPr><m:t>{t}</m:t></m:r>"#);
        let cases: Vec<(&str, String)> = vec![
            ("a+b", m(&r("a+b"))),
            ("a - b", m(&r("a−b"))),
            ("a/b", m(&frac(&r("a"), &r("b")))),
            ("(a+b)/c", m(&frac(&r("a+b"), &r("c")))),
            ("1/2+x", m(&format!("{}{}", frac(&r("1"), &r("2")), r("+x")))),
            ("2πr/3", m(&frac(&r("2πr"), &r("3")))),
            ("a¦b", m(r#"<m:f><m:fPr><m:type m:val="noBar"/></m:fPr><m:num><m:r><m:t>a</m:t></m:r></m:num><m:den><m:r><m:t>b</m:t></m:r></m:den></m:f>"#)),
            ("x^2", m(&sup(&r("x"), &r("2")))),
            ("x_i", m(&sub(&r("x"), &r("i")))),
            ("x_i^2", m(&format!("<m:sSubSup><m:e>{}</m:e><m:sub>{}</m:sub><m:sup>{}</m:sup></m:sSubSup>", r("x"), r("i"), r("2")))),
            ("e^(-x)", m(&sup(&r("e"), &r("−x")))),
            ("e^-x", m(&sup(&r("e"), &r("−x")))),
            ("x^10", m(&sup(&r("x"), &r("10")))),
            ("f'", m(&sup(&r("f"), &r("′")))),
            ("√x", m(&sqrt(&r("x")))),
            ("√(x+1)", m(&sqrt(&r("x+1")))),
            ("√(3&x)", m(&format!("<m:rad><m:deg>{}</m:deg><m:e>{}</m:e></m:rad>", r("3"), r("x")))),
            ("∛8", m(&format!("<m:rad><m:deg>{}</m:deg><m:e>{}</m:e></m:rad>", r("3"), r("8")))),
            ("(a+b)", m(&format!("<m:d><m:e>{}</m:e></m:d>", r("a+b")))),
            ("[x]", m(&format!(r#"<m:d><m:dPr><m:begChr m:val="["/><m:endChr m:val="]"/></m:dPr><m:e>{}</m:e></m:d>"#, r("x")))),
            ("|x|", m(&format!(r#"<m:d><m:dPr><m:begChr m:val="|"/><m:endChr m:val="|"/></m:dPr><m:e>{}</m:e></m:d>"#, r("x")))),
            ("(a∣b)", m(&format!("<m:d><m:e>{}</m:e><m:e>{}</m:e></m:d>", r("a"), r("b")))),
            ("∑_(k=1)^n k", m(&format!(r#"<m:nary><m:naryPr><m:chr m:val="∑"/><m:limLoc m:val="undOvr"/></m:naryPr><m:sub>{}</m:sub><m:sup>{}</m:sup><m:e>{}</m:e></m:nary>"#, r("k=1"), r("n"), r("k")))),
            ("∫_0^1▒x dx", m(&format!(r#"<m:nary><m:naryPr><m:chr m:val="∫"/><m:limLoc m:val="subSup"/></m:naryPr><m:sub>{}</m:sub><m:sup>{}</m:sup><m:e>{}</m:e></m:nary>{}"#, r("0"), r("1"), r("x"), r("dx")))),
            ("∫x", m(&format!(r#"<m:nary><m:naryPr><m:chr m:val="∫"/><m:limLoc m:val="subSup"/><m:subHide m:val="1"/><m:supHide m:val="1"/></m:naryPr><m:sub></m:sub><m:sup></m:sup><m:e>{}</m:e></m:nary>"#, r("x")))),
            ("sin x", m(&format!("<m:func><m:fName>{}</m:fName><m:e>{}</m:e></m:func>", up("sin"), r("x")))),
            ("lim_(x→0) x", m(&format!("<m:func><m:fName><m:limLow><m:e>{}</m:e><m:lim>{}</m:lim></m:limLow></m:fName><m:e>{}</m:e></m:func>", up("lim"), r("x→0"), r("x")))),
            ("lim_(x→0) sin x/x", m(&format!("<m:func><m:fName><m:limLow><m:e>{}</m:e><m:lim>{}</m:lim></m:limLow></m:fName><m:e>{}</m:e></m:func>", up("lim"), r("x→0"), frac(&format!("<m:func><m:fName>{}</m:fName><m:e>{}</m:e></m:func>", up("sin"), r("x")), &r("x"))))),
            ("sin x/x", m(&frac(&format!("<m:func><m:fName>{}</m:fName><m:e>{}</m:e></m:func>", up("sin"), r("x")), &r("x")))),
            ("log_2 x", m(&format!("<m:func><m:fName><m:sSub><m:e>{}</m:e><m:sub>{}</m:sub></m:sSub></m:fName><m:e>{}</m:e></m:func>", up("log"), r("2"), r("x")))),
            ("■(1&0@0&1)", m(&format!("<m:m><m:mr><m:e>{}</m:e><m:e>{}</m:e></m:mr><m:mr><m:e>{}</m:e><m:e>{}</m:e></m:mr></m:m>", r("1"), r("0"), r("0"), r("1")))),
            ("█(x+y=3@x-y=1)", m(&format!("<m:eqArr><m:e>{}</m:e><m:e>{}</m:e></m:eqArr>", r("x+y=3"), r("x−y=1")))),
            ("x\u{302}", m(&format!(r#"<m:acc><m:accPr><m:chr m:val="{}"/></m:accPr><m:e>{}</m:e></m:acc>"#, '\u{302}', r("x")))),
            ("v\u{20D7}", m(&format!(r#"<m:acc><m:accPr><m:chr m:val="{}"/></m:accPr><m:e>{}</m:e></m:acc>"#, '\u{20D7}', r("v")))),
            ("x\u{305}", m(&format!(r#"<m:bar><m:barPr><m:pos m:val="top"/></m:barPr><m:e>{}</m:e></m:bar>"#, r("x")))),
            ("\"if \" x>0", m(&format!(r#"<m:r><m:rPr><m:nor/></m:rPr><m:t xml:space="preserve">if </m:t></m:r>{}"#, r("x&gt;0")))),
            ("\\alpha+\\beta", m(&r("α+β"))),
            ("\\sqrt x \\le \\infty", m(&format!("{}{}", sqrt(&r("x")), r("≤∞")))),
            ("a<b", m(&r("a&lt;b"))),
        ];
        for (src, want) in cases {
            assert_eq!(to_omml(src), want, "{src}");
        }
    }

    // source: quadratic formula and Pythagoras typed in UnicodeMath (UTN #28 §2).
    #[test]
    fn nested_expressions() {
        let x = to_omml("x=(-b±√(b^2-4ac))/2a");
        assert!(
            x.starts_with("<m:oMath><m:r><m:t>x=</m:t></m:r><m:f><m:num>"),
            "{x}"
        );
        assert!(x.contains("<m:rad>") && x.contains("<m:sSup><m:e><m:r><m:t>b</m:t></m:r></m:e><m:sup><m:r><m:t>2</m:t></m:r></m:sup></m:sSup>"), "{x}");
        assert!(
            x.ends_with("<m:den><m:r><m:t>2a</m:t></m:r></m:den></m:f></m:oMath>"),
            "{x}"
        );
        assert_eq!(to_omml("a^2+b^2=c^2").matches("<m:sSup>").count(), 3);
    }

    proptest::proptest! {
        #[test]
        fn never_panics_and_writes_balanced_xml(s in "\\PC{0,40}") {
            let x = to_omml(&s);
            proptest::prop_assert!(x.starts_with("<m:oMath>") && x.ends_with("</m:oMath>"));
            proptest::prop_assert_eq!(x.matches("<m:e>").count(), x.matches("</m:e>").count());
        }
    }
}
