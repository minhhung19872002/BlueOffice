//! Equations in BlueDoc (`docs/parity/BACKLOG.md` CORE-MATH-007): insert from the UnicodeMath linear
//! format or LaTeX (`bo-math`), read the selected equation back in the linear format, and switch an
//! equation between the built-up (professional) and the linear form as Office Math does — the linear
//! form is a math zone holding the linear text in one run (ECMA-376 Part 1, §22.1.2.98 r).

use bo_math::{Node, Style};
use serde::Serialize;

/// The equation the selection covers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EquationInfo {
    /// Its UnicodeMath linear form.
    pub linear: String,
    /// A display equation (`m:oMathPara`) rather than an inline one.
    pub display: bool,
    /// Currently shown in the linear form.
    pub is_linear: bool,
}

/// `<m:oMath>` (or `<m:oMathPara>` around it when `display`) with the math namespace declared on the
/// element itself, so the zone is well-formed in any part (a new document's root does not declare `m`).
#[must_use]
pub fn wrap(omath: &str, display: bool) -> String {
    let ns = bo_math::omml::NS;
    if display {
        format!(r#"<m:oMathPara xmlns:m="{ns}">{omath}</m:oMathPara>"#)
    } else {
        omath.replacen("<m:oMath>", &format!(r#"<m:oMath xmlns:m="{ns}">"#), 1)
    }
}

/// [`wrap`], a display zone keeping its properties (`m:oMathParaPr`, ECMA-376 Part 1, §22.1.2.79, as
/// XML) when it had some.
#[must_use]
pub fn wrap_with(omath: &str, display: bool, para_pr: Option<&str>) -> String {
    match (display, para_pr) {
        (true, Some(pr)) => format!(
            r#"<m:oMathPara xmlns:m="{}">{pr}{omath}</m:oMathPara>"#,
            bo_math::omml::NS
        ),
        _ => wrap(omath, display),
    }
}

/// OMML of an equation typed as UnicodeMath (or LaTeX when `latex`).
#[must_use]
pub fn build(text: &str, latex: bool, display: bool) -> String {
    let nodes = if latex {
        bo_math::latex::parse(text)
    } else {
        bo_math::unicode_math::parse(text)
    };
    wrap(&bo_math::to_omml(&nodes), display)
}

/// BlueOffice's equation templates (UnicodeMath linear format): well-known formulas, written for
/// BlueOffice; the UI names them by id (`equation.templates.<id>`).
pub const TEMPLATES: &[(&str, &str)] = &[
    ("pythagoras", "a^2+b^2=c^2"),
    ("quadratic", "x=(-b±√(b^2-4ac))/2a"),
    ("circle", "S=πr^2"),
    ("sphere", "V=4/3 πr^3"),
    ("binomial", "(a+b)^n=∑_(k=0)^n (n¦k) a^(n-k) b^k"),
    ("geometric", "∑_(k=0)^(n-1) q^k=(1-q^n)/(1-q)"),
    ("squares", "∑_(k=1)^n k^2=(n(n+1)(2n+1))/6"),
    ("exp", "e^x=∑_(n=0)^∞ x^n/n!"),
    ("limit", "lim_(n→∞) (1+1/n)^n=e"),
    ("euler", "e^(iπ)+1=0"),
    ("gauss", "∫_(-∞)^∞ e^(-x^2) dx=√π"),
    ("derivative", "f'(x)=lim_(h→0) (f(x+h)-f(x))/h"),
    ("trig", "sin^2 α+cos^2 α=1"),
    ("cosine", "c^2=a^2+b^2-2ab cos γ"),
    ("matrix", "A=[■(a&b@c&d)], det A=ad-bc"),
];

/// `true` for an `m:oMathPara` zone.
#[must_use]
pub fn is_display(xml: &str) -> bool {
    let name = xml
        .trim_start()
        .strip_prefix('<')
        .and_then(|s| {
            s.split(|c: char| c.is_whitespace() || c == '>' || c == '/')
                .next()
        })
        .unwrap_or("");
    name.rsplit(':').next() == Some("oMathPara")
}

/// The linear text when the zone is shown in the linear form (one ordinary math run), else `None`.
fn linear_text(nodes: &[Node]) -> Option<&str> {
    match nodes {
        [Node::Run(t, Style::Math, _)] if t.chars().any(|c| "/^_√▒■█(".contains(c)) => {
            Some(t)
        }
        _ => None,
    }
}

/// What the equation editor shows for an equation.
#[must_use]
pub fn info(xml: &str) -> Option<EquationInfo> {
    let nodes = bo_math::from_omml(xml)?;
    let is_linear = linear_text(&nodes).is_some();
    let linear = match linear_text(&nodes) {
        Some(t) => t.to_owned(),
        None => bo_math::to_linear(&nodes),
    };
    Some(EquationInfo {
        linear,
        display: is_display(xml),
        is_linear,
    })
}

/// The zone switched to the linear form (`linear`) or built up again; `None` when it already is.
#[must_use]
pub fn reformat(xml: &str, linear: bool) -> Option<String> {
    let nodes = bo_math::from_omml(xml)?;
    let display = is_display(xml);
    match (linear, linear_text(&nodes)) {
        (true, None) => {
            let text = bo_math::to_linear(&nodes);
            Some(wrap(
                &bo_math::to_omml(&[Node::run(text, Style::Math)]),
                display,
            ))
        }
        (false, Some(t)) => Some(build(t, false, display)),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::edit::Command;
    use crate::Engine;

    fn engine(text: &str) -> Engine {
        let mut e = Engine::with_bundled_fonts();
        e.load(crate::plain_text_document(text), "t.docx").unwrap();
        e
    }

    // source: UnicodeMath (UTN #28) input, OMML per ECMA-376 Part 1 §22.1.
    #[test]
    fn insert_read_back_switch_and_save() {
        let mut e = engine("Ta có ");
        e.command(Command::Move {
            motion: crate::nav::Motion::DocEnd,
            extend: false,
        })
        .unwrap();
        let r = e
            .command(Command::InsertEquation {
                text: "x^2+1".into(),
                latex: false,
                display: false,
                replace: false,
                numbered: None,
            })
            .unwrap();
        let info = r.equation.clone().unwrap();
        assert_eq!(info.linear, "x^2+1");
        assert!(!info.display && !info.is_linear);
        // Linear form: one run holding the text; built up again: the superscript is back.
        let r = e.command(Command::EquationFormat { linear: true }).unwrap();
        assert!(r.equation.as_ref().unwrap().is_linear);
        let r = e
            .command(Command::EquationFormat { linear: false })
            .unwrap();
        assert_eq!(r.equation.as_ref().unwrap().linear, "x^2+1");
        // Replace it with a LaTeX display equation.
        let r = e
            .command(Command::InsertEquation {
                text: r"\frac{a}{b}".into(),
                latex: true,
                display: true,
                replace: true,
                numbered: None,
            })
            .unwrap();
        let info = r.equation.unwrap();
        assert_eq!(info.linear, "a/b");
        assert!(info.display);
        // Saved as a well-formed zone that reads back the same.
        let bytes = e.save_docx().unwrap();
        let mut back = Engine::with_bundled_fonts();
        back.open(&bytes, "t.docx").unwrap();
        back.command(Command::Move {
            motion: crate::nav::Motion::DocEnd,
            extend: false,
        })
        .unwrap();
        let r = back.command(Command::Equation).unwrap();
        assert_eq!(r.equation.unwrap().linear, "a/b");
    }

    // After inserting an equation or switching its form, the caret is just after it (not the
    // equation selected): Enter starts a new paragraph and keeps the equation, typing goes there.
    #[test]
    fn enter_after_an_inserted_or_switched_equation_keeps_it() {
        for switch in [false, true] {
            let mut e = engine("Ta có ");
            e.command(Command::Move {
                motion: crate::nav::Motion::DocEnd,
                extend: false,
            })
            .unwrap();
            let r = e
                .command(Command::InsertEquation {
                    text: "x^2+1".into(),
                    latex: false,
                    display: false,
                    replace: false,
                    numbered: None,
                })
                .unwrap();
            let after = bo_edit::Pos::new(0, "Ta có ".len() + '\u{FFFC}'.len_utf8());
            assert_eq!(r.selection, bo_edit::Selection::caret(after));
            assert!(r.equation.is_some());
            if switch {
                let r = e.command(Command::EquationFormat { linear: true }).unwrap();
                assert_eq!(r.selection, bo_edit::Selection::caret(after));
                assert!(r.equation.unwrap().is_linear);
            }
            let r = e.command(Command::SplitParagraph).unwrap();
            assert_eq!(r.selection.focus.para, 1);
            let r = e
                .command(Command::InsertText {
                    text: "tiếp".into(),
                })
                .unwrap();
            assert!(r.equation.is_none(), "switch {switch}");
            let s = e.session.as_ref().unwrap();
            assert_eq!(s.para_text(0).unwrap(), "Ta có \u{FFFC}");
            assert_eq!(s.para_text(1).unwrap(), "tiếp");
        }
    }

    #[test]
    fn display_detection_and_wrapping() {
        let x = build("a/b", false, true);
        assert!(is_display(&x));
        assert!(x.starts_with(r#"<m:oMathPara xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"><m:oMath>"#));
        let x = build("a", false, false);
        assert!(!is_display(&x));
        assert!(bo_model::index::is_math_xml(&x));
    }
}
