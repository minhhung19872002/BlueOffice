//! [`Node`] tree → UnicodeMath linear format (Unicode Technical Note #28), the inverse of
//! [`crate::unicode_math::parse`]: what the equation editor shows when an equation is switched to the
//! linear form. Operands of fractions, scripts and radicals get parentheses unless they are one entity.

use crate::{Node, Style};

/// The linear form of `nodes`.
#[must_use]
pub fn to_linear(nodes: &[Node]) -> String {
    let mut s = String::new();
    for (i, n) in nodes.iter().enumerate() {
        let next_starts_word = nodes.get(i + 1).is_some_and(starts_with_letter);
        write(&mut s, n);
        // A function or n-ary operand followed by letters needs a space to stay apart.
        if matches!(n, Node::Func { .. } | Node::Nary { .. }) && next_starts_word {
            s.push(' ');
        }
    }
    s
}

fn starts_with_letter(n: &Node) -> bool {
    match n {
        Node::Run(t, _) => t.chars().next().is_some_and(char::is_alphanumeric),
        _ => false,
    }
}

/// `true` when the operand reads back as one entity without parentheses.
fn atomic(nodes: &[Node]) -> bool {
    match nodes {
        // One character, or a number.
        [Node::Run(t, Style::Math)] => {
            t.chars().count() == 1 || t.chars().all(|c| c.is_ascii_digit() || c == '.')
        }
        // Literal text and function names read back whole; brackets, radicals and arrays delimit
        // themselves.
        []
        | [Node::Run(_, Style::Normal | Style::Upright)
        | Node::Delim { .. }
        | Node::Rad { .. }
        | Node::Matrix(_)
        | Node::EqArray(_)] => true,
        _ => false,
    }
}

fn operand(s: &mut String, nodes: &[Node]) {
    if atomic(nodes) {
        s.push_str(&to_linear(nodes));
    } else {
        s.push('(');
        s.push_str(&to_linear(nodes));
        s.push(')');
    }
}

fn scripts(s: &mut String, sub: Option<&Vec<Node>>, sup: Option<&Vec<Node>>) {
    if let Some(b) = sub {
        s.push('_');
        operand(s, b);
    }
    if let Some(p) = sup {
        s.push('^');
        operand(s, p);
    }
}

fn write(s: &mut String, n: &Node) {
    match n {
        Node::Run(t, Style::Normal) => {
            s.push('"');
            s.push_str(t);
            s.push('"');
        }
        Node::Run(t, _) => s.push_str(t),
        Node::Frac { num, den, bar } => {
            operand(s, num);
            s.push(if *bar { '/' } else { '¦' });
            operand(s, den);
        }
        Node::Scripts { base, sub, sup } => {
            operand(s, base);
            scripts(s, sub.as_ref(), sup.as_ref());
        }
        Node::Rad { deg, body } => match deg {
            Some(d) if matches!(d.as_slice(), [Node::Run(t, _)] if t == "3") => {
                s.push('∛');
                operand(s, body);
            }
            Some(d) if matches!(d.as_slice(), [Node::Run(t, _)] if t == "4") => {
                s.push('∜');
                operand(s, body);
            }
            Some(d) => {
                s.push_str("√(");
                s.push_str(&to_linear(d));
                s.push('&');
                s.push_str(&to_linear(body));
                s.push(')');
            }
            None => {
                s.push('√');
                operand(s, body);
            }
        },
        Node::Nary {
            chr,
            sub,
            sup,
            body,
        } => {
            s.push(*chr);
            scripts(s, sub.as_ref(), sup.as_ref());
            if !body.is_empty() {
                s.push('▒');
                operand(s, body);
            }
        }
        Node::Delim { open, close, parts } => {
            if let Some(o) = open {
                s.push(*o);
            }
            for (i, p) in parts.iter().enumerate() {
                if i > 0 {
                    s.push('∣');
                }
                s.push_str(&to_linear(p));
            }
            if let Some(c) = close {
                s.push(*c);
            }
        }
        Node::Func { name, arg } => {
            s.push_str(&to_linear(name));
            if !arg.is_empty() {
                s.push(' ');
                // lim / max / min read the whole term that follows; other functions one operand.
                let limit = match name.as_slice() {
                    [Node::Limit { .. }] => true,
                    [Node::Run(t, _)] => crate::unicode_math::LIMIT_FUNCTIONS.contains(&t.as_str()),
                    _ => false,
                };
                if limit {
                    s.push_str(&to_linear(arg));
                } else {
                    operand(s, arg);
                }
            }
        }
        Node::Limit { base, lim, upper } => {
            s.push_str(&to_linear(base));
            s.push(if *upper { '^' } else { '_' });
            operand(s, lim);
        }
        Node::Matrix(rows) => {
            s.push_str("■(");
            for (i, r) in rows.iter().enumerate() {
                if i > 0 {
                    s.push('@');
                }
                for (j, c) in r.iter().enumerate() {
                    if j > 0 {
                        s.push('&');
                    }
                    s.push_str(&to_linear(c));
                }
            }
            s.push(')');
        }
        Node::EqArray(rows) => {
            s.push_str("█(");
            for (i, r) in rows.iter().enumerate() {
                if i > 0 {
                    s.push('@');
                }
                s.push_str(&to_linear(r));
            }
            s.push(')');
        }
        Node::Accent { chr, base } => {
            operand(s, base);
            s.push(*chr);
        }
        Node::Bar { top, base } => {
            operand(s, base);
            s.push(if *top { '\u{305}' } else { '\u{332}' });
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::unicode_math::{parse, to_omml};

    // source: the UnicodeMath examples of the parser's table (UTN #28): linear → tree → linear → tree
    // gives the same OMML.
    #[test]
    fn linear_round_trip() {
        for src in [
            "a+b",
            "a/b",
            "(a+b)/c",
            "1/2+x",
            "a¦b",
            "x^2",
            "x_i^2",
            "e^(-x)",
            "x^10",
            "f'",
            "√x",
            "√(x+1)",
            "√(5&x)",
            "∛8",
            "(a+b)",
            "[x]",
            "|x|",
            "(a∣b)",
            "∑_(k=1)^n k",
            "∫_0^1▒x dx",
            "sin x",
            "lim_(x→0) sin x/x",
            "log_2 x",
            "■(1&0@0&1)",
            "█(x+y=3@x-y=1)",
            "x\u{302}+v\u{20D7}",
            "x\u{305}",
            "\"if \" x>0",
            "x=(-b±√(b^2-4ac))/2a",
            "∑_(k=1)^n k^2=(n(n+1)(2n+1))/6",
            "A=[■(a&b@c&d)]",
        ] {
            let lin = super::to_linear(&parse(src));
            assert_eq!(to_omml(&lin), to_omml(src), "{src} → {lin}");
        }
    }

    // source: same examples read back from the OMML the parser writes (ECMA-376 §22.1).
    #[test]
    fn omml_to_linear() {
        for (src, want) in [
            ("(a+b)/c", "(a+b)/c"),
            ("x_i^2", "x_i^2"),
            ("√(x+1)", "√(x+1)"),
            ("∑_(k=1)^n k", "∑_(k=1)^n▒k"),
            ("sin x", "sin x"),
            ("■(1&0@0&1)", "■(1&0@0&1)"),
        ] {
            let xml = to_omml(src);
            let nodes = crate::read::from_omml(&xml.replace(
                "<m:oMath>",
                r#"<m:oMath xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">"#,
            ))
            .unwrap();
            assert_eq!(super::to_linear(&nodes), want, "{src}");
        }
    }
}
