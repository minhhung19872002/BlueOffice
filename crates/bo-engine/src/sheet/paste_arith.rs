//! Paste Special operations: add, subtract, multiply or divide the copied cells into the
//! destination (round 36). Rules measured on the reference through its object model
//! (`Range.PasteSpecial` with `Operation`, see `docs/ui/REPORT-sheet.md` round 36):
//!
//! - number ∘ number gives a constant (`5 + 3` → `8`); a blank cell counts as 0 on either
//!   side (`5 × blank` → `0`, `5 ÷ blank` → `#DIV/0!`), two blanks stay blank;
//! - a destination that is text (unless it reads as a number), a logical or an error
//!   constant is left alone, and so is any destination when the source is text, a logical
//!   or an error constant;
//! - a destination formula becomes `=(formula)∘operand`; pasting formulas (All /
//!   Formulas) with a source formula gives `=dest∘(source formula)`; pasting values uses
//!   the source formula's result (a text or error result leaves the destination alone).

use bo_model::sheet::{Cell, CellError, CellValue, Formula};
use serde::{Deserialize, Serialize};

/// Arithmetic applied by a paste (Paste Special › Operation).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PasteOp {
    /// Replace the destination (an ordinary paste).
    #[default]
    None,
    /// Destination + source.
    Add,
    /// Destination − source.
    Subtract,
    /// Destination × source.
    Multiply,
    /// Destination ÷ source.
    Divide,
}

impl PasteOp {
    fn symbol(self) -> &'static str {
        match self {
            Self::None | Self::Add => "+",
            Self::Subtract => "-",
            Self::Multiply => "*",
            Self::Divide => "/",
        }
    }

    fn apply(self, a: f64, b: f64) -> CellValue {
        let v = match self {
            Self::None => b,
            Self::Add => a + b,
            Self::Subtract => a - b,
            Self::Multiply => a * b,
            Self::Divide => {
                if b == 0.0 {
                    return CellValue::Error(CellError::Div0);
                }
                a / b
            }
        };
        if v.is_finite() {
            CellValue::Number(v)
        } else {
            CellValue::Error(CellError::Num)
        }
    }
}

/// One side of the operation.
enum Operand {
    Number(f64),
    /// A formula (text without `=`).
    Formula(String),
    /// Text, logical or error: the destination is left alone.
    Other,
}

fn constant(v: &CellValue) -> Operand {
    match v {
        CellValue::Empty => Operand::Number(0.0),
        CellValue::Number(n) => Operand::Number(*n),
        CellValue::Text(t) => t
            .trim()
            .parse::<f64>()
            .map_or(Operand::Other, Operand::Number),
        _ => Operand::Other,
    }
}

/// Literal of `n` inside a formula (locale-invariant, as formulas are stored).
fn literal(n: f64) -> String {
    let s = format!("{n}");
    if n < 0.0 {
        format!("({s})")
    } else {
        s
    }
}

/// The destination cell after pasting `src` onto `old` with `op`. `src_formula` is the
/// source's formula already moved to the destination (`None` when values are pasted or
/// the source has none); `style` the style the paste gives the cell. `None` leaves the
/// cell blank, `Some(old)` unchanged.
pub(crate) fn combine(
    op: PasteOp,
    old: Option<&Cell>,
    src: Option<&Cell>,
    src_formula: Option<String>,
    style: u32,
) -> Option<Cell> {
    let blank = |c: Option<&Cell>| {
        c.is_none_or(|c| c.formula.is_none() && matches!(c.value, CellValue::Empty))
    };
    if blank(old) && blank(src) {
        return old
            .cloned()
            .filter(|c| c.style != 0 || style != 0)
            .map(|mut c| {
                c.style = style;
                c
            });
    }
    let right = match src_formula.filter(|t| !t.is_empty()) {
        Some(t) => Operand::Formula(t),
        None => constant(src.map_or(&CellValue::Empty, |c| &c.value)),
    };
    let left = match old
        .and_then(|c| c.formula.as_ref())
        .filter(|f| !f.text.is_empty())
    {
        Some(f) => Operand::Formula(f.text.clone()),
        None => constant(old.map_or(&CellValue::Empty, |c| &c.value)),
    };
    let mut cell = old.cloned().unwrap_or_default();
    let sym = op.symbol();
    match (left, right) {
        (Operand::Other, _) | (_, Operand::Other) => return old.cloned(),
        (Operand::Number(a), Operand::Number(b)) => {
            cell.formula = None;
            cell.value = op.apply(a, b);
        }
        (l, r) => {
            let text = |o: Operand| match o {
                Operand::Number(n) => literal(n),
                Operand::Formula(t) => format!("({t})"),
                Operand::Other => String::new(),
            };
            let f = format!("{}{sym}{}", text(l), text(r));
            cell.formula = Some(Formula {
                text: f,
                ..Formula::default()
            });
            // Calculated by the paste's recalculation.
            cell.value = CellValue::Empty;
        }
    }
    cell.style = style;
    Some(cell)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(n: f64) -> Cell {
        Cell::new(CellValue::Number(n))
    }

    fn text(t: &str) -> Cell {
        Cell::new(CellValue::Text(t.into()))
    }

    fn formula(t: &str) -> Cell {
        Cell {
            formula: Some(Formula {
                text: t.into(),
                ..Formula::default()
            }),
            ..Cell::default()
        }
    }

    fn value(c: Option<Cell>) -> CellValue {
        c.map_or(CellValue::Empty, |c| c.value)
    }

    fn ftext(c: Option<Cell>) -> String {
        c.and_then(|c| c.formula)
            .map(|f| f.text)
            .unwrap_or_default()
    }

    #[test]
    fn constants_as_the_reference() {
        use PasteOp::{Add, Divide, Multiply, Subtract};
        let r = |op, d: Option<Cell>, s: Option<Cell>| {
            value(combine(op, d.as_ref(), s.as_ref(), None, 0))
        };
        assert_eq!(
            r(Add, Some(num(5.0)), Some(num(3.0))),
            CellValue::Number(8.0)
        );
        assert_eq!(r(Subtract, None, Some(num(3.0))), CellValue::Number(-3.0));
        assert_eq!(r(Add, Some(num(5.0)), None), CellValue::Number(5.0));
        assert_eq!(r(Multiply, Some(num(5.0)), None), CellValue::Number(0.0));
        assert_eq!(
            r(Divide, Some(num(5.0)), None),
            CellValue::Error(CellError::Div0)
        );
        assert_eq!(r(Divide, None, Some(num(3.0))), CellValue::Number(0.0));
        assert_eq!(
            r(Multiply, Some(text("7")), Some(num(3.0))),
            CellValue::Number(21.0)
        );
        // Text / logical destinations and text sources leave the destination alone.
        assert_eq!(
            r(Add, Some(text("y")), Some(num(3.0))),
            CellValue::Text("y".into())
        );
        assert_eq!(
            r(Add, Some(num(5.0)), Some(text("x"))),
            CellValue::Number(5.0)
        );
        assert_eq!(
            r(Add, Some(Cell::new(CellValue::Bool(true))), Some(num(3.0))),
            CellValue::Bool(true)
        );
        assert!(combine(Add, None, None, None, 0).is_none());
    }

    #[test]
    fn formulas_as_the_reference() {
        use PasteOp::{Add, Subtract};
        let d = formula("1+1");
        assert_eq!(
            ftext(combine(Add, Some(&d), Some(&num(3.0)), None, 0)),
            "(1+1)+3"
        );
        assert_eq!(
            ftext(combine(
                Subtract,
                Some(&num(5.0)),
                Some(&formula("F1*2")),
                Some("F1*2".into()),
                0
            )),
            "5-(F1*2)"
        );
        assert_eq!(
            ftext(combine(
                Add,
                Some(&d),
                Some(&formula("F1*2")),
                Some("F1*2".into()),
                0
            )),
            "(1+1)+(F1*2)"
        );
        // Values: the source formula's result.
        let mut s = formula("F1*2");
        s.value = CellValue::Number(8.0);
        assert_eq!(ftext(combine(Add, Some(&d), Some(&s), None, 0)), "(1+1)+8");
        assert_eq!(
            value(combine(Add, Some(&num(5.0)), Some(&s), None, 0)),
            CellValue::Number(13.0)
        );
        s.value = CellValue::Error(CellError::Div0);
        assert_eq!(
            value(combine(Add, Some(&num(5.0)), Some(&s), None, 0)),
            CellValue::Number(5.0)
        );
    }
}
