//! The EQ field (ECMA-376 Part 1, §17.16.5.17 EQ) as an Office Math zone (§22.1.2.77 oMath), so
//! the math layout draws it.
//!
//! An EQ code is text with switches: `\f(num,den)` fraction, `\r(n,x)` radical, `\s\up n(x)`
//! script, `\x\to(x)` box, `\o(a,b)` overstrike, `\b\lc\{(x)` brackets, `\a\co2(…)` array,
//! `\l(a,b)` list, `\i\su(lo,hi,x)` integral / sum, `\d\fo n()` displacement. Arguments are
//! between parentheses, separated by `,` (or `;` where the decimal separator is a comma);
//! `\,` `\;` `\(` `\)` `\\` are literal characters. Switches nest inside arguments.
//!
//! Approximations (the math layout has no free placement):
//! - `\o` (overstrike): the first element is the base; each further single character becomes a
//!   combining mark drawn over it when one exists (`/` → U+0338, `|` → U+20D2, `~` → U+0303, …),
//!   otherwise an accent (§22.1.2.1 acc) over the base.
//! - `\d` (displacement): `\fo n` becomes about `n` points of space, `\li` draws a line under
//!   that space (§22.1.2.7 bar); `\ba` (backwards) cannot move left in a math zone and is not
//!   drawn.
//! - `\s` with several elements and no `\up` / `\do`: the elements stacked (§22.1.2.34 eqArr);
//!   `\ai n` / `\di n` (space above / below each line) become the row spacing of the stack
//!   (§22.1.2.90 rSpRule "exactly" + §22.1.2.89 rSp, points). With a single element or a script
//!   they have no effect.
//! - `\a`: `\vs n` becomes the row spacing (rSpRule 3 + rSp, points) and `\hs n` the column gap
//!   (§22.1.2.13 cGpRule 3 + §22.1.2.12 cGp, twips) of the matrix.

/// OMML namespace (§22.1).
const NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/math";

/// The EQ instruction `instr` (with or without its `EQ` keyword) as an `m:oMath` zone; `None`
/// when the code cannot be read (the field then shows its cached result).
#[must_use]
pub fn to_omml(instr: &str) -> Option<String> {
    let t = instr.trim_start();
    let body = match t.get(..2) {
        Some(k) if k.eq_ignore_ascii_case("EQ") => &t[2..],
        _ => t,
    };
    let chars: Vec<char> = body.trim().chars().collect();
    if chars.is_empty() {
        return None;
    }
    let mut p = Parser { s: &chars, i: 0 };
    let inner = p.seq(false)?;
    if p.i != chars.len() {
        return None;
    }
    Some(format!("<m:oMath xmlns:m=\"{NS}\">{inner}</m:oMath>"))
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            _ => o.push(c),
        }
    }
    o
}

/// §22.1.2.98 r: upright text (§22.1.2.90 sty `p`; EQ text keeps the run's own look).
fn run(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    format!(
        "<m:r><m:rPr><m:sty m:val=\"p\"/></m:rPr><m:t xml:space=\"preserve\">{}</m:t></m:r>",
        esc(text)
    )
}

/// `<m:name>` wrapping `inner`.
fn el(name: &str, inner: &str) -> String {
    format!("<m:{name}>{inner}</m:{name}>")
}

/// An `m:val` property element.
fn val(name: &str, v: &str) -> String {
    format!("<m:{name} m:val=\"{}\"/>", esc(v))
}

/// A switch option: its name and its value (a number or a character).
#[derive(Debug, Clone, PartialEq)]
enum Opt {
    Flag(String),
    Num(String, f32),
    Chr(String, char),
}

/// Options that take a number, and those that take a character.
const NUM_OPTS: [&str; 9] = ["co", "vs", "hs", "up", "do", "ai", "di", "fo", "ba"];
const CHR_OPTS: [&str; 5] = ["lc", "rc", "bc", "fc", "vc"];

struct Parser<'a> {
    s: &'a [char],
    i: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<char> {
        self.s.get(self.i).copied()
    }

    fn skip_spaces(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.i += 1;
        }
    }

    /// Text and switches up to the end, or (`in_args`) up to an argument separator / `)`.
    fn seq(&mut self, in_args: bool) -> Option<String> {
        let mut out = String::new();
        let mut text = String::new();
        while let Some(c) = self.peek() {
            if in_args && matches!(c, ',' | ';' | ')') {
                break;
            }
            if c == ')' || c == '(' {
                // An unbalanced parenthesis outside a switch.
                if c == '(' {
                    return None;
                }
                if !in_args {
                    return None;
                }
            }
            if c == '\\' {
                let next = self.s.get(self.i + 1).copied()?;
                if next.is_ascii_alphabetic() {
                    out.push_str(&run(&std::mem::take(&mut text)));
                    self.i += 1;
                    out.push_str(&self.switch()?);
                } else {
                    // `\,` `\(` `\)` `\\`: the character itself.
                    text.push(next);
                    self.i += 2;
                }
                continue;
            }
            text.push(c);
            self.i += 1;
        }
        out.push_str(&run(&text));
        Some(out)
    }

    /// The letters of a switch or option name (lower-case).
    fn name(&mut self, max: usize) -> String {
        let mut n = String::new();
        while n.len() < max {
            match self.peek() {
                Some(c) if c.is_ascii_alphabetic() => {
                    n.push(c.to_ascii_lowercase());
                    self.i += 1;
                }
                _ => break,
            }
        }
        n
    }

    /// The options after a switch (`\co2`, `\lc\{`, `\up 8`, `\to`), up to its `(`.
    fn options(&mut self) -> Option<Vec<Opt>> {
        let mut opts = Vec::new();
        loop {
            self.skip_spaces();
            if self.peek() != Some('\\') {
                break;
            }
            let save = self.i;
            self.i += 1;
            let n = self.name(2);
            if n.len() != 2 {
                // Not an option (another switch would need an argument list first).
                self.i = save;
                return None;
            }
            if NUM_OPTS.contains(&n.as_str()) {
                self.skip_spaces();
                let start = self.i;
                while self
                    .peek()
                    .is_some_and(|c| c.is_ascii_digit() || c == '.' || c == '-')
                {
                    self.i += 1;
                }
                let v: String = self.s[start..self.i].iter().collect();
                opts.push(Opt::Num(n, v.parse().unwrap_or(0.0)));
            } else if CHR_OPTS.contains(&n.as_str()) {
                // `\lc\{`: the character, usually escaped.
                if self.peek() == Some('\\') {
                    self.i += 1;
                }
                let c = self.peek()?;
                self.i += 1;
                opts.push(Opt::Chr(n, c));
            } else {
                opts.push(Opt::Flag(n));
            }
        }
        Some(opts)
    }

    /// `(a,b,…)`: the arguments of a switch.
    fn args(&mut self) -> Option<Vec<String>> {
        self.skip_spaces();
        if self.peek() != Some('(') {
            return None;
        }
        self.i += 1;
        let mut out = Vec::new();
        loop {
            out.push(self.seq(true)?);
            match self.peek()? {
                ')' => {
                    self.i += 1;
                    return Some(out);
                }
                _ => self.i += 1,
            }
        }
    }

    /// One switch (after its backslash) with its options and arguments.
    fn switch(&mut self) -> Option<String> {
        let sw = self.name(1);
        let opts = self.options()?;
        let args = self.args()?;
        let flag = |n: &str| opts.iter().any(|o| matches!(o, Opt::Flag(f) if f == n));
        let num = |n: &str| {
            opts.iter().find_map(|o| match o {
                Opt::Num(k, v) if k == n => Some(*v),
                _ => None,
            })
        };
        let chr = |n: &str| {
            opts.iter().find_map(|o| match o {
                Opt::Chr(k, c) if k == n => Some(*c),
                _ => None,
            })
        };
        let arg = |i: usize| args.get(i).cloned().unwrap_or_default();
        Some(match sw.as_str() {
            // \f(numerator,denominator): fraction (§22.1.2.36 f).
            "f" => format!("<m:f>{}{}</m:f>", el("num", &arg(0)), el("den", &arg(1))),
            // \r(x) / \r(n,x): radical, with its degree (§22.1.2.88 rad).
            "r" => {
                if args.len() >= 2 {
                    format!("<m:rad>{}{}</m:rad>", el("deg", &arg(0)), el("e", &arg(1)))
                } else {
                    format!(
                        "<m:rad><m:radPr>{}</m:radPr><m:deg/>{}</m:rad>",
                        val("degHide", "1"),
                        el("e", &arg(0))
                    )
                }
            }
            // \s: superscript (\up n), subscript (\do n), else the elements stacked
            // (§22.1.2.112 sSup, §22.1.2.110 sSub, §22.1.2.34 eqArr).
            "s" => {
                let all: String = args.concat();
                if num("up").is_some() {
                    format!("<m:sSup><m:e/>{}</m:sSup>", el("sup", &all))
                } else if num("do").is_some() {
                    format!("<m:sSub><m:e/>{}</m:sSub>", el("sub", &all))
                } else if args.len() > 1 {
                    let rows: String = args.iter().map(|a| el("e", a)).collect();
                    // \ai n / \di n: space above / below each line → the row spacing
                    // (CT_EqArrPr order: rSpRule, rSp).
                    let (ai, di) = (num("ai"), num("di"));
                    if ai.is_some() || di.is_some() {
                        let extra = ai.unwrap_or(0.0).max(0.0) + di.unwrap_or(0.0).max(0.0);
                        let pr = format!("{}{}", val("rSpRule", "3"), val("rSp", &whole(extra)));
                        format!("<m:eqArr>{}{rows}</m:eqArr>", el("eqArrPr", &pr))
                    } else {
                        el("eqArr", &rows)
                    }
                } else {
                    all
                }
            }
            // \x: box, or only the borders asked for (\to \bo \le \ri; §22.1.2.13 borderBox).
            "x" => {
                let sides = [
                    ("to", "hideTop"),
                    ("bo", "hideBot"),
                    ("le", "hideLeft"),
                    ("ri", "hideRight"),
                ];
                let any = sides.iter().any(|(o, _)| flag(o));
                let pr: String = if any {
                    sides
                        .iter()
                        .filter(|(o, _)| !flag(o))
                        .map(|(_, h)| val(h, "1"))
                        .collect()
                } else {
                    String::new()
                };
                let pr = if pr.is_empty() {
                    String::new()
                } else {
                    el("borderBoxPr", &pr)
                };
                format!("<m:borderBox>{pr}{}</m:borderBox>", el("e", &args.concat()))
            }
            // \o: overstrike (approximated, see the module notes).
            "o" => overstrike(&args),
            // \b: brackets (\lc \rc \bc; §22.1.2.24 d).
            "b" => {
                let (l, r) = match (chr("bc"), chr("lc"), chr("rc")) {
                    (Some(b), _, _) => (b.to_string(), closing(b).to_string()),
                    (None, None, None) => ("(".into(), ")".into()),
                    (None, l, r) => (
                        l.map(String::from).unwrap_or_default(),
                        r.map(String::from).unwrap_or_default(),
                    ),
                };
                let es: String = args.iter().map(|a| el("e", a)).collect();
                format!(
                    "<m:d><m:dPr>{}{}{}</m:dPr>{es}</m:d>",
                    val("begChr", &l),
                    val("sepChr", ","),
                    val("endChr", &r)
                )
            }
            // \a: array, row by row (\co n columns; \al \ac \ar alignment; §22.1.2.60 m).
            "a" => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let cols = num("co").map_or(1, |c| c.max(1.0) as usize);
                let jc = if flag("al") {
                    "left"
                } else if flag("ar") {
                    "right"
                } else {
                    "center"
                };
                let mut rows = String::new();
                for chunk in args.chunks(cols) {
                    let mut cells: String = chunk.iter().map(|a| el("e", a)).collect();
                    for _ in chunk.len()..cols {
                        cells.push_str("<m:e/>");
                    }
                    rows.push_str(&el("mr", &cells));
                }
                // \vs n: row spacing in points, \hs n: column gap (CT_MPr order: rSpRule,
                // cGpRule, rSp, cGp, mcs).
                let vs = num("vs").map(|v| v.max(0.0));
                let hs = num("hs").map(|v| v.max(0.0));
                let mut spacing = String::new();
                if vs.is_some() {
                    spacing.push_str(&val("rSpRule", "3"));
                }
                if hs.is_some() {
                    spacing.push_str(&val("cGpRule", "3"));
                }
                if let Some(v) = vs {
                    spacing.push_str(&val("rSp", &whole(v)));
                }
                if let Some(h) = hs {
                    spacing.push_str(&val("cGp", &whole(h * 20.0)));
                }
                format!(
                    "<m:m><m:mPr>{spacing}<m:mcs><m:mc><m:mcPr>{}{}</m:mcPr></m:mc></m:mcs></m:mPr>{rows}</m:m>",
                    val("count", &cols.to_string()),
                    val("mcJc", jc)
                )
            }
            // \l: the elements as a list.
            "l" => args.join(&run(",")),
            // \i(lower,upper,integrand): integral, \su sum, \pr product, \fc / \vc a character of
            // its own, \in limits beside the sign (§22.1.2.70 nary).
            "i" => {
                let sign = chr("fc").or(chr("vc")).unwrap_or(if flag("su") {
                    '∑'
                } else if flag("pr") {
                    '∏'
                } else {
                    '∫'
                });
                let loc = if flag("in") { "subSup" } else { "undOvr" };
                format!(
                    "<m:nary><m:naryPr>{}{}</m:naryPr>{}{}{}</m:nary>",
                    val("chr", &sign.to_string()),
                    val("limLoc", loc),
                    el("sub", &arg(0)),
                    el("sup", &arg(1)),
                    el("e", &arg(2))
                )
            }
            // \d: displacement (\fo n points forward; \li a line under the space, §22.1.2.7
            // bar at the bottom; \ba not drawn).
            "d" => {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let n = (num("fo").unwrap_or(0.0).max(0.0) / 4.0).round() as usize;
                let space = run(&"\u{2004}".repeat(n));
                let space = if flag("li") && !space.is_empty() {
                    format!(
                        "<m:bar>{}{}</m:bar>",
                        el("barPr", &val("pos", "bot")),
                        el("e", &space)
                    )
                } else {
                    space
                };
                format!("{space}{}", args.concat())
            }
            _ => return None,
        })
    }
}

/// A non-negative size as the whole number an `ST_UnSignedInteger` attribute takes.
fn whole(v: f32) -> String {
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let n = v.clamp(0.0, 31_680.0).round() as u32;
    n.to_string()
}

/// The closing bracket of an opening one (`\bc\{` draws `{` and `}`).
fn closing(c: char) -> char {
    match c {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        '<' => '>',
        '⟨' => '⟩',
        other => other,
    }
}

/// Combining mark drawn over a character for an overstruck one.
fn combining(c: char) -> Option<char> {
    Some(match c {
        '/' => '\u{0338}',
        '|' => '\u{20D2}',
        '~' => '\u{0303}',
        '^' => '\u{0302}',
        '¨' => '\u{0308}',
        '¯' | '_' => '\u{0304}',
        '´' | '\'' => '\u{0301}',
        '`' => '\u{0300}',
        '¸' => '\u{0327}',
        '.' => '\u{0307}',
        _ => return None,
    })
}

/// Plain text of an argument made of one run only.
fn single_text(arg: &str) -> Option<String> {
    let rest =
        arg.strip_prefix("<m:r><m:rPr><m:sty m:val=\"p\"/></m:rPr><m:t xml:space=\"preserve\">")?;
    let t = rest.strip_suffix("</m:t></m:r>")?;
    (!t.contains('<')).then(|| t.to_owned())
}

/// \o(a,b,…): `a` with the others struck over it.
fn overstrike(args: &[String]) -> String {
    let Some((base, rest)) = args.split_first() else {
        return String::new();
    };
    let mut out = base.clone();
    for a in rest {
        let t = single_text(a);
        let mark = t.as_deref().and_then(|t| {
            let mut cs = t.chars();
            match (cs.next(), cs.next()) {
                (Some(c), None) => combining(c),
                _ => None,
            }
        });
        if let (Some(m), Some(bt)) = (mark, single_text(&out)) {
            out = run(&format!("{bt}{m}"));
        } else if let Some(t) = t {
            out = format!(
                "<m:acc><m:accPr>{}</m:accPr>{}</m:acc>",
                val("chr", &t),
                el("e", &out)
            );
        } else {
            out.push_str(a);
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn body(instr: &str) -> String {
        let x = to_omml(instr).unwrap();
        let open = format!("<m:oMath xmlns:m=\"{NS}\">");
        x.strip_prefix(&open)
            .and_then(|r| r.strip_suffix("</m:oMath>"))
            .unwrap()
            .to_owned()
    }

    fn r(t: &str) -> String {
        run(t)
    }

    #[test]
    fn fraction_and_radical() {
        assert_eq!(
            body("EQ \\f(1,2)"),
            format!(
                "<m:f><m:num>{}</m:num><m:den>{}</m:den></m:f>",
                r("1"),
                r("2")
            )
        );
        // Lower-case keyword, `;` separator, text around.
        assert_eq!(
            body("eq x=\\F(a;b)"),
            format!(
                "{}<m:f><m:num>{}</m:num><m:den>{}</m:den></m:f>",
                r("x="),
                r("a"),
                r("b")
            )
        );
        assert_eq!(
            body("EQ \\r(3,x)"),
            format!(
                "<m:rad><m:deg>{}</m:deg><m:e>{}</m:e></m:rad>",
                r("3"),
                r("x")
            )
        );
        assert!(body("EQ \\r(x)").contains("<m:degHide m:val=\"1\"/>"));
    }

    #[test]
    fn nested_switches_and_escapes() {
        // A fraction inside a radical.
        assert_eq!(
            body("EQ \\r(\\f(a,b))"),
            format!(
                "<m:rad><m:radPr><m:degHide m:val=\"1\"/></m:radPr><m:deg/><m:e><m:f><m:num>{}</m:num><m:den>{}</m:den></m:f></m:e></m:rad>",
                r("a"),
                r("b")
            )
        );
        // `\,` and `\(` are characters, not separators / groups; XML is escaped.
        assert_eq!(
            body("EQ \\f(1\\,5,a<b)"),
            format!(
                "<m:f><m:num>{}</m:num><m:den>{}</m:den></m:f>",
                r("1,5"),
                r("a<b")
            )
        );
        assert!(body("EQ a\\(b\\)").contains("a(b)"));
        // Unbalanced or unknown: no math (the cached result shows).
        assert_eq!(to_omml("EQ \\f(1,2"), None);
        assert_eq!(to_omml("EQ \\q(1)"), None);
        assert_eq!(to_omml("EQ"), None);
    }

    #[test]
    fn scripts() {
        assert_eq!(
            body("EQ x\\s\\up8(2)"),
            format!("{}<m:sSup><m:e/><m:sup>{}</m:sup></m:sSup>", r("x"), r("2"))
        );
        assert_eq!(
            body("EQ H\\s\\do4(2)O"),
            format!(
                "{}<m:sSub><m:e/><m:sub>{}</m:sub></m:sSub>{}",
                r("H"),
                r("2"),
                r("O")
            )
        );
        assert_eq!(
            body("EQ \\s(a,b)"),
            format!(
                "<m:eqArr><m:e>{}</m:e><m:e>{}</m:e></m:eqArr>",
                r("a"),
                r("b")
            )
        );
    }

    #[test]
    fn boxes_and_brackets() {
        assert_eq!(
            body("EQ \\x(a)"),
            format!("<m:borderBox><m:e>{}</m:e></m:borderBox>", r("a"))
        );
        // Only the top and bottom borders: left and right hidden.
        assert_eq!(
            body("EQ \\x\\to\\bo(a)"),
            format!(
                "<m:borderBox><m:borderBoxPr><m:hideLeft m:val=\"1\"/><m:hideRight m:val=\"1\"/></m:borderBoxPr><m:e>{}</m:e></m:borderBox>",
                r("a")
            )
        );
        let paren = body("EQ \\b(x)");
        assert!(paren.contains("<m:begChr m:val=\"(\"/>"), "{paren}");
        assert!(paren.contains("<m:endChr m:val=\")\"/>"));
        let both = body("EQ \\b\\bc\\{(x)");
        assert!(
            both.contains("<m:begChr m:val=\"{\"/>") && both.contains("<m:endChr m:val=\"}\"/>")
        );
        // Only a left brace (cases).
        let left = body("EQ \\b\\lc\\{(x)");
        assert!(
            left.contains("<m:begChr m:val=\"{\"/>") && left.contains("<m:endChr m:val=\"\"/>")
        );
    }

    #[test]
    fn arrays_lists_integrals() {
        let a = body("EQ \\a\\al\\co2(1,2,3)");
        assert!(
            a.contains("<m:count m:val=\"2\"/><m:mcJc m:val=\"left\"/>"),
            "{a}"
        );
        assert!(a.ends_with(&format!(
            "<m:mr><m:e>{}</m:e><m:e>{}</m:e></m:mr><m:mr><m:e>{}</m:e><m:e/></m:mr></m:m>",
            r("1"),
            r("2"),
            r("3")
        )));
        assert_eq!(
            body("EQ \\l(a,b)"),
            format!("{}{}{}", r("a"), r(","), r("b"))
        );
        let s = body("EQ \\i\\su(1,n,k)");
        assert!(
            s.contains("<m:chr m:val=\"∑\"/><m:limLoc m:val=\"undOvr\"/>"),
            "{s}"
        );
        assert!(s.contains(&format!(
            "<m:sub>{}</m:sub><m:sup>{}</m:sup><m:e>{}</m:e>",
            r("1"),
            r("n"),
            r("k")
        )));
        let i = body("EQ \\i\\in(0,1,x)");
        assert!(i.contains("<m:chr m:val=\"∫\"/><m:limLoc m:val=\"subSup\"/>"));
        let fc = body("EQ \\i\\fc\\∮(a,b,c)");
        assert!(fc.contains("<m:chr m:val=\"∮\"/>"));
    }

    #[test]
    fn overstrike_and_displacement() {
        // `=` struck by `/`: a combining long solidus over it (≠).
        assert_eq!(body("EQ \\o(=,/)"), r("=\u{0338}"));
        // No combining equivalent: an accent over the base.
        assert_eq!(
            body("EQ \\o(O,+)"),
            format!(
                "<m:acc><m:accPr><m:chr m:val=\"+\"/></m:accPr><m:e>{}</m:e></m:acc>",
                r("O")
            )
        );
        assert_eq!(
            body("EQ a\\d\\fo8()b"),
            format!("{}{}{}", r("a"), r("\u{2004}\u{2004}"), r("b"))
        );
        // \li: the space underlined.
        assert_eq!(
            body("EQ a\\d\\fo8\\li()b"),
            format!(
                "{}<m:bar><m:barPr><m:pos m:val=\"bot\"/></m:barPr><m:e>{}</m:e></m:bar>{}",
                r("a"),
                r("\u{2004}\u{2004}"),
                r("b")
            )
        );
    }

    /// ECMA-376 Part 1, §17.16.5.17: \a\vs / \hs and \s\ai / \di spacing options.
    #[test]
    fn spacing_options() {
        let a = body("EQ \\a\\co2\\vs6\\hs4(1,2,3,4)");
        assert!(
            a.starts_with("<m:m><m:mPr><m:rSpRule m:val=\"3\"/><m:cGpRule m:val=\"3\"/><m:rSp m:val=\"6\"/><m:cGp m:val=\"80\"/><m:mcs>"),
            "{a}"
        );
        // Only one of them.
        let v = body("EQ \\a\\vs 3(1,2)");
        assert!(
            v.starts_with("<m:m><m:mPr><m:rSpRule m:val=\"3\"/><m:rSp m:val=\"3\"/><m:mcs>"),
            "{v}"
        );
        // No option: no spacing elements.
        assert!(body("EQ \\a(1,2)").starts_with("<m:m><m:mPr><m:mcs>"));
        let s = body("EQ \\s\\ai4\\di2(a,b)");
        assert_eq!(
            s,
            format!(
                "<m:eqArr><m:eqArrPr><m:rSpRule m:val=\"3\"/><m:rSp m:val=\"6\"/></m:eqArrPr><m:e>{}</m:e><m:e>{}</m:e></m:eqArr>",
                r("a"),
                r("b")
            )
        );
        // A script with \ai keeps its shape.
        assert!(body("EQ x\\s\\up8\\ai2(2)").contains("<m:sSup>"));
    }
}
