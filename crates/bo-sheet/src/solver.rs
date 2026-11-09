//! Solver: linear programs over worksheet formulas (simplex), with integer and binary
//! variables by branch and bound.
//!
//! The model is read from the formulas themselves: the objective and every constraint's left
//! side are evaluated on the formula engine with each variable cell moved by one, giving the
//! linear coefficients; a check at another point rejects formulas that are not linear. The
//! linear program is solved with a dense two-phase simplex (Bland's rule, so it cannot cycle);
//! integer variables are branched on depth first, pruned by the relaxation's bound.

use bo_formula::Value;
use bo_model::sheet::{CellRange, CellValue, Workbook};
use serde::{Deserialize, Serialize};

use crate::formula::load;

/// What the objective should do.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Goal {
    /// Maximise.
    Max,
    /// Minimise.
    Min,
    /// Reach a value.
    Value(f64),
}

/// Relation of a constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Relation {
    /// `<=`
    Le,
    /// `>=`
    Ge,
    /// `=`
    Eq,
    /// The cells take whole values.
    Int,
    /// The cells take 0 or 1.
    Bin,
}

/// One constraint: `lhs relation rhs` cell by cell (`rhs` a number or a range of the same
/// size); `Int` / `Bin` apply to variable cells and have no right side.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Constraint {
    /// Left side: cells holding formulas or variable cells.
    pub lhs: CellRange,
    /// The relation.
    pub relation: Relation,
    /// Right side: a constant…
    #[serde(default)]
    pub rhs_value: Option<f64>,
    /// …or cells.
    #[serde(default)]
    pub rhs: Option<CellRange>,
}

/// A Solver model on one sheet.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Model {
    /// The objective cell.
    pub objective: (u32, u32),
    /// Maximise, minimise or reach a value.
    pub goal: Goal,
    /// The variable cells.
    pub variables: Vec<CellRange>,
    /// The constraints.
    #[serde(default)]
    pub constraints: Vec<Constraint>,
    /// Variables without a lower bound constraint are non-negative.
    #[serde(default = "yes")]
    pub non_negative: bool,
}

fn yes() -> bool {
    true
}

/// Outcome of a solve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Status {
    /// An optimal solution was found.
    Optimal,
    /// No point satisfies the constraints.
    Infeasible,
    /// The objective can grow without limit.
    Unbounded,
    /// The objective or a constraint is not a linear function of the variables.
    NotLinear,
    /// A cell gives an error or no number.
    Error,
}

/// A solve result.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Solution {
    /// What happened.
    pub status: Status,
    /// The objective's value at the solution.
    pub objective: f64,
    /// Variable cells and their values (sheet order of `variables`).
    pub values: Vec<((u32, u32), f64)>,
}

// ------------------------------------------------------------------ linear model

/// An affine function `c · x + c0` of the variables.
#[derive(Debug, Clone)]
struct Affine {
    c: Vec<f64>,
    c0: f64,
}

fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => Some(*n),
        Value::Bool(b) => Some(f64::from(u8::from(*b))),
        Value::Empty => Some(0.0),
        _ => None,
    }
}

/// Coefficients of cells `cells` by moving each variable by one from `base`, checked at a
/// second point (`base + 2` for every variable).
fn linearise(
    fw: &mut bo_formula::Workbook,
    sheet: usize,
    vars: &[(u32, u32)],
    base: &[f64],
    cells: &[(u32, u32)],
) -> Result<Vec<Affine>, Status> {
    let set = |fw: &mut bo_formula::Workbook, x: &[f64]| {
        for (&(r, c), &v) in vars.iter().zip(x) {
            fw.set_value(sheet, r, c, Value::Number(v));
        }
        fw.recalculate();
    };
    let read = |fw: &bo_formula::Workbook| -> Result<Vec<f64>, Status> {
        cells
            .iter()
            .map(|&(r, c)| number(&fw.value(sheet, r, c)).ok_or(Status::Error))
            .collect()
    };
    set(fw, base);
    let f0 = read(fw)?;
    let mut coef = vec![vec![0.0; vars.len()]; cells.len()];
    for j in 0..vars.len() {
        let mut x = base.to_vec();
        x[j] += 1.0;
        set(fw, &x);
        let f1 = read(fw)?;
        for (i, row) in coef.iter_mut().enumerate() {
            row[j] = f1[i] - f0[i];
        }
    }
    // Linearity check at another point.
    let probe: Vec<f64> = base
        .iter()
        .enumerate()
        .map(|(j, b)| b + 2.0 + j as f64 * 0.5)
        .collect();
    set(fw, &probe);
    let fp = read(fw)?;
    for (i, row) in coef.iter().enumerate() {
        let predicted = f0[i]
            + row
                .iter()
                .zip(probe.iter().zip(base))
                .map(|(c, (p, b))| c * (p - b))
                .sum::<f64>();
        let scale = 1.0 + predicted.abs().max(fp[i].abs());
        if (predicted - fp[i]).abs() > 1e-7 * scale {
            return Err(Status::NotLinear);
        }
    }
    set(fw, base);
    Ok(coef
        .into_iter()
        .zip(f0)
        .map(|(c, v)| {
            let c0 = v - c.iter().zip(base).map(|(a, b)| a * b).sum::<f64>();
            Affine { c, c0 }
        })
        .collect())
}

/// A row `a · x (<=|>=|=) b`.
#[derive(Debug, Clone)]
struct Row {
    a: Vec<f64>,
    rel: Relation,
    b: f64,
}

// ------------------------------------------------------------------ simplex

enum Lp {
    Optimal(Vec<f64>, f64),
    Infeasible,
    Unbounded,
}

/// Maximise `c · x` subject to `rows`, `x ≥ 0`, by the two-phase simplex with Bland's rule.
fn simplex(c: &[f64], rows: &[Row]) -> Lp {
    const EPS: f64 = 1e-9;
    let n = c.len();
    let m = rows.len();
    // Normalise to b ≥ 0.
    let rows: Vec<Row> = rows
        .iter()
        .map(|r| {
            if r.b < 0.0 {
                Row {
                    a: r.a.iter().map(|v| -v).collect(),
                    rel: match r.rel {
                        Relation::Le => Relation::Ge,
                        Relation::Ge => Relation::Le,
                        other => other,
                    },
                    b: -r.b,
                }
            } else {
                r.clone()
            }
        })
        .collect();
    // Columns: x (n), slack/surplus (one per inequality), artificial (one per ≥ / =).
    let slacks = rows.iter().filter(|r| r.rel != Relation::Eq).count();
    let artificials = rows.iter().filter(|r| r.rel != Relation::Le).count();
    let width = n + slacks + artificials;
    let mut t = vec![vec![0.0; width + 1]; m];
    let mut basis = vec![0usize; m];
    let (mut s, mut a) = (n, n + slacks);
    for (i, r) in rows.iter().enumerate() {
        t[i][..n].copy_from_slice(&r.a);
        t[i][width] = r.b;
        match r.rel {
            Relation::Le => {
                t[i][s] = 1.0;
                basis[i] = s;
                s += 1;
            }
            Relation::Ge => {
                t[i][s] = -1.0;
                s += 1;
                t[i][a] = 1.0;
                basis[i] = a;
                a += 1;
            }
            _ => {
                t[i][a] = 1.0;
                basis[i] = a;
                a += 1;
            }
        }
    }
    let pivot = |t: &mut Vec<Vec<f64>>,
                 obj: &mut Vec<f64>,
                 basis: &mut Vec<usize>,
                 r: usize,
                 col: usize| {
        let p = t[r][col];
        for v in &mut t[r] {
            *v /= p;
        }
        for i in 0..t.len() {
            if i != r && t[i][col] != 0.0 {
                let f = t[i][col];
                let pr = t[r].clone();
                for (v, pv) in t[i].iter_mut().zip(&pr) {
                    *v -= f * pv;
                }
            }
        }
        if obj[col] != 0.0 {
            let f = obj[col];
            for (v, pv) in obj.iter_mut().zip(&t[r]) {
                *v -= f * pv;
            }
        }
        basis[r] = col;
    };
    // Run simplex iterations minimising the reduced-cost row `obj` (last entry: −value).
    let run = |t: &mut Vec<Vec<f64>>,
               obj: &mut Vec<f64>,
               basis: &mut Vec<usize>,
               allowed: usize|
     -> bool {
        for _ in 0..50_000 {
            // Bland: the lowest-index column with a negative reduced cost.
            let Some(col) = (0..allowed).find(|&j| obj[j] < -EPS) else {
                return true;
            };
            let mut best: Option<(usize, f64)> = None;
            for i in 0..t.len() {
                if t[i][col] > EPS {
                    let ratio = t[i][width] / t[i][col];
                    match best {
                        Some((bi, br))
                            if ratio > br + EPS
                                || ((ratio - br).abs() <= EPS && basis[i] > basis[bi]) => {}
                        _ => best = Some((i, ratio)),
                    }
                }
            }
            let Some((r, _)) = best else {
                return false;
            };
            pivot(t, obj, basis, r, col);
        }
        true
    };
    // Phase 1: minimise the sum of artificial variables.
    let mut obj = vec![0.0; width + 1];
    obj[n + slacks..width].fill(1.0);
    for i in 0..m {
        if basis[i] >= n + slacks {
            for j in 0..=width {
                obj[j] -= t[i][j];
            }
        }
    }
    run(&mut t, &mut obj, &mut basis, width);
    if -obj[width] > 1e-7 {
        return Lp::Infeasible;
    }
    // Drive artificial variables out of the basis where possible.
    for i in 0..m {
        if basis[i] >= n + slacks {
            if let Some(col) = (0..n + slacks).find(|&j| t[i][j].abs() > EPS) {
                let mut dummy = vec![0.0; width + 1];
                pivot(&mut t, &mut dummy, &mut basis, i, col);
            }
        }
    }
    // Phase 2: maximise c · x ⇔ minimise −c · x.
    let mut obj = vec![0.0; width + 1];
    for j in 0..n {
        obj[j] = -c[j];
    }
    for i in 0..m {
        let bj = basis[i];
        if bj < n && obj[bj] != 0.0 {
            let f = obj[bj];
            for j in 0..=width {
                obj[j] -= f * t[i][j];
            }
        }
    }
    if !run(&mut t, &mut obj, &mut basis, n + slacks) {
        return Lp::Unbounded;
    }
    let mut x = vec![0.0; n];
    for i in 0..m {
        if basis[i] < n {
            x[basis[i]] = t[i][width];
        }
    }
    let value = c.iter().zip(&x).map(|(a, b)| a * b).sum();
    Lp::Optimal(x, value)
}

/// Maximise with integer variables `ints` by branch and bound (depth first).
fn branch_and_bound(c: &[f64], rows: &[Row], ints: &[bool]) -> Lp {
    let mut best: Option<(Vec<f64>, f64)> = None;
    let mut unbounded = false;
    let mut stack: Vec<Vec<Row>> = vec![Vec::new()];
    let mut nodes = 0;
    while let Some(extra) = stack.pop() {
        nodes += 1;
        if nodes > 20_000 {
            break;
        }
        let mut all = rows.to_vec();
        all.extend(extra.iter().cloned());
        match simplex(c, &all) {
            Lp::Infeasible => {}
            Lp::Unbounded => unbounded = true,
            Lp::Optimal(x, v) => {
                if best.as_ref().is_some_and(|(_, bv)| v <= *bv + 1e-9) {
                    continue;
                }
                let frac = (0..x.len()).find(|&j| ints[j] && (x[j] - x[j].round()).abs() > 1e-6);
                match frac {
                    None => {
                        let x: Vec<f64> = x
                            .iter()
                            .zip(ints)
                            .map(|(v, i)| if *i { v.round() } else { *v })
                            .collect();
                        best = Some((x, v));
                    }
                    Some(j) => {
                        let unit = |k: usize| {
                            let mut a = vec![0.0; c.len()];
                            a[k] = 1.0;
                            a
                        };
                        let mut down = extra.clone();
                        down.push(Row {
                            a: unit(j),
                            rel: Relation::Le,
                            b: x[j].floor(),
                        });
                        let mut up = extra;
                        up.push(Row {
                            a: unit(j),
                            rel: Relation::Ge,
                            b: x[j].ceil(),
                        });
                        stack.push(up);
                        stack.push(down);
                    }
                }
            }
        }
    }
    match best {
        Some((x, v)) => Lp::Optimal(x, v),
        None if unbounded => Lp::Unbounded,
        None => Lp::Infeasible,
    }
}

// ------------------------------------------------------------------ the sheet model

fn cells_of(r: &CellRange) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    for row in r.first.row..=r.last.row {
        for col in r.first.col..=r.last.col {
            out.push((row, col));
        }
    }
    out
}

/// Solve `model` on sheet `sheet` of `wb` (the workbook is not changed).
#[must_use]
pub fn solve(wb: &Workbook, sheet: usize, model: &Model) -> Solution {
    let vars: Vec<(u32, u32)> = model.variables.iter().flat_map(cells_of).collect();
    let fail = |status| Solution {
        status,
        objective: 0.0,
        values: Vec::new(),
    };
    if vars.is_empty() {
        return fail(Status::Error);
    }
    let ws = &wb.sheets[sheet];
    let base: Vec<f64> = vars
        .iter()
        .map(|k| match ws.cells.get(k).map(|c| &c.value) {
            Some(CellValue::Number(n)) => *n,
            _ => 0.0,
        })
        .collect();
    let mut fw = load(wb, None);
    fw.recalculate_all();
    let var_index = |cell: (u32, u32)| vars.iter().position(|v| *v == cell);
    // Formula cells to linearise: the objective and every constraint left / right side.
    let mut probe: Vec<(u32, u32)> = vec![model.objective];
    for con in &model.constraints {
        probe.extend(cells_of(&con.lhs));
        if let Some(r) = &con.rhs {
            probe.extend(cells_of(r));
        }
    }
    probe.sort_unstable();
    probe.dedup();
    let affine = match linearise(&mut fw, sheet, &vars, &base, &probe) {
        Ok(a) => a,
        Err(s) => return fail(s),
    };
    let f = |cell: (u32, u32)| -> &Affine {
        let i = probe.binary_search(&cell).unwrap_or(0);
        &affine[i]
    };
    let n = vars.len();
    // Free variables are split x = x⁺ − x⁻ when they may be negative.
    let free: Vec<bool> = vec![!model.non_negative; n];
    let width = n + free.iter().filter(|f| **f).count();
    let expand = |a: &[f64]| -> Vec<f64> {
        let mut out = a.to_vec();
        for (j, is_free) in free.iter().enumerate() {
            if *is_free {
                out.push(-a[j]);
            }
        }
        out
    };
    let mut rows: Vec<Row> = Vec::new();
    let mut ints = vec![false; width];
    for con in &model.constraints {
        let lhs = cells_of(&con.lhs);
        match con.relation {
            Relation::Int | Relation::Bin => {
                for cell in lhs {
                    let Some(j) = var_index(cell) else { continue };
                    ints[j] = true;
                    if con.relation == Relation::Bin {
                        let mut a = vec![0.0; n];
                        a[j] = 1.0;
                        rows.push(Row {
                            a: expand(&a),
                            rel: Relation::Le,
                            b: 1.0,
                        });
                    }
                }
            }
            rel => {
                let rhs_cells = con.rhs.as_ref().map(cells_of);
                for (k, cell) in lhs.iter().enumerate() {
                    let l = f(*cell);
                    let (rc, r0) = match (&rhs_cells, con.rhs_value) {
                        (Some(rc), _) => {
                            let Some(&rcell) = rc.get(k).or_else(|| rc.first()) else {
                                return fail(Status::Error);
                            };
                            let r = f(rcell);
                            (r.c.clone(), r.c0)
                        }
                        (None, Some(v)) => (vec![0.0; n], v),
                        (None, None) => return fail(Status::Error),
                    };
                    // (l.c − r.c) · x  rel  r0 − l.c0
                    let a: Vec<f64> = l.c.iter().zip(&rc).map(|(x, y)| x - y).collect();
                    rows.push(Row {
                        a: expand(&a),
                        rel,
                        b: r0 - l.c0,
                    });
                }
            }
        }
    }
    let obj = f(model.objective).clone();
    let c: Vec<f64> = match model.goal {
        Goal::Max => expand(&obj.c),
        Goal::Min => expand(&obj.c.iter().map(|v| -v).collect::<Vec<_>>()),
        Goal::Value(v) => {
            rows.push(Row {
                a: expand(&obj.c),
                rel: Relation::Eq,
                b: v - obj.c0,
            });
            vec![0.0; width]
        }
    };
    let lp = if ints.iter().any(|i| *i) {
        branch_and_bound(&c, &rows, &ints)
    } else {
        simplex(&c, &rows)
    };
    match lp {
        Lp::Infeasible => fail(Status::Infeasible),
        Lp::Unbounded => fail(Status::Unbounded),
        Lp::Optimal(x, _) => {
            let mut k = n;
            let values: Vec<((u32, u32), f64)> = vars
                .iter()
                .enumerate()
                .map(|(j, cell)| {
                    let mut v = x[j];
                    if free[j] {
                        v -= x[k];
                        k += 1;
                    }
                    // Clean rounding noise (15 significant digits).
                    (*cell, format!("{v:.14e}").parse().unwrap_or(v))
                })
                .collect();
            let objective = obj.c0
                + obj
                    .c
                    .iter()
                    .zip(&values)
                    .map(|(a, (_, v))| a * v)
                    .sum::<f64>();
            Solution {
                status: Status::Optimal,
                objective: format!("{objective:.14e}").parse().unwrap_or(objective),
                values,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::{Cell, CellRef, Worksheet};

    fn book(cells: &[(&str, &str)]) -> Workbook {
        let mut ws = Worksheet::new("S");
        for (a1, input) in cells {
            let r = CellRef::parse(a1).unwrap_or_default();
            if let Some(f) = input.strip_prefix('=') {
                ws.set_cell(r.row, r.col, Cell::default().with_formula(f));
            } else {
                ws.set_a1(a1, CellValue::Number(input.parse().unwrap_or(0.0)));
            }
        }
        let mut wb = Workbook::default();
        wb.sheets.push(ws);
        wb
    }

    fn range(a: &str) -> CellRange {
        CellRange::parse(a).unwrap()
    }

    fn con(lhs: &str, relation: Relation, rhs: f64) -> Constraint {
        Constraint {
            lhs: range(lhs),
            relation,
            rhs_value: Some(rhs),
            rhs: None,
        }
    }

    fn value(s: &Solution, a1: &str) -> f64 {
        let r = CellRef::parse(a1).unwrap();
        s.values
            .iter()
            .find(|(c, _)| *c == (r.row, r.col))
            .unwrap()
            .1
    }

    #[test]
    fn classic_linear_program() {
        // Maximise 3x + 5y s.t. x ≤ 4, 2y ≤ 12, 3x + 2y ≤ 18 → x = 2, y = 6, value 36.
        let wb = book(&[
            ("A1", "0"),
            ("B1", "0"),
            ("C1", "=3*A1+5*B1"),
            ("C2", "=2*B1"),
            ("C3", "=3*A1+2*B1"),
        ]);
        let m = Model {
            objective: (0, 2),
            goal: Goal::Max,
            variables: vec![range("A1:B1")],
            constraints: vec![
                con("A1", Relation::Le, 4.0),
                con("C2", Relation::Le, 12.0),
                con("C3", Relation::Le, 18.0),
            ],
            non_negative: true,
        };
        let s = solve(&wb, 0, &m);
        assert_eq!(s.status, Status::Optimal);
        assert!((s.objective - 36.0).abs() < 1e-9, "{s:?}");
        assert!((value(&s, "A1") - 2.0).abs() < 1e-9);
        assert!((value(&s, "B1") - 6.0).abs() < 1e-9);
    }

    #[test]
    fn minimise_with_greater_than_and_equality() {
        // Diet: minimise 2x + 3y s.t. x + y ≥ 4, x + 3y ≥ 6, x = y + 1 → x = 2.5, y = 1.5, 9.5.
        let wb = book(&[
            ("A1", "0"),
            ("B1", "0"),
            ("C1", "=2*A1+3*B1"),
            ("C2", "=A1+B1"),
            ("C3", "=A1+3*B1"),
            ("C4", "=A1-B1"),
        ]);
        let m = Model {
            objective: (0, 2),
            goal: Goal::Min,
            variables: vec![range("A1:B1")],
            constraints: vec![
                con("C2", Relation::Ge, 4.0),
                con("C3", Relation::Ge, 6.0),
                con("C4", Relation::Eq, 1.0),
            ],
            non_negative: true,
        };
        let s = solve(&wb, 0, &m);
        assert_eq!(s.status, Status::Optimal, "{s:?}");
        assert!((s.objective - 9.5).abs() < 1e-9, "{s:?}");
        assert!((value(&s, "A1") - 2.5).abs() < 1e-9);
    }

    #[test]
    fn integer_and_binary_variables() {
        // Maximise 5x + 4y s.t. 6x + 4y ≤ 24, x + 2y ≤ 6, integers → LP optimum (3, 1.5)
        // value 21; the integer optimum is x = 4, y = 0, value 20.
        let wb = book(&[
            ("A1", "0"),
            ("B1", "0"),
            ("C1", "=5*A1+4*B1"),
            ("C2", "=6*A1+4*B1"),
            ("C3", "=A1+2*B1"),
        ]);
        let mut m = Model {
            objective: (0, 2),
            goal: Goal::Max,
            variables: vec![range("A1:B1")],
            constraints: vec![con("C2", Relation::Le, 24.0), con("C3", Relation::Le, 6.0)],
            non_negative: true,
        };
        let lp = solve(&wb, 0, &m);
        assert!((lp.objective - 21.0).abs() < 1e-9, "{lp:?}");
        m.constraints.push(Constraint {
            lhs: range("A1:B1"),
            relation: Relation::Int,
            rhs_value: None,
            rhs: None,
        });
        let ip = solve(&wb, 0, &m);
        assert_eq!(ip.status, Status::Optimal);
        assert!((ip.objective - 20.0).abs() < 1e-9, "{ip:?}");
        assert!((value(&ip, "A1") - 4.0).abs() < 1e-9);
        // Knapsack with binaries: values 10, 13, 7, weights 5, 6, 4, capacity 10 → items 2
        // and 3 (13 + 7 = 20).
        let wb = book(&[
            ("A1", "0"),
            ("B1", "0"),
            ("C1", "0"),
            ("D1", "=10*A1+13*B1+7*C1"),
            ("D2", "=5*A1+6*B1+4*C1"),
        ]);
        let m = Model {
            objective: (0, 3),
            goal: Goal::Max,
            variables: vec![range("A1:C1")],
            constraints: vec![
                con("D2", Relation::Le, 10.0),
                Constraint {
                    lhs: range("A1:C1"),
                    relation: Relation::Bin,
                    rhs_value: None,
                    rhs: None,
                },
            ],
            non_negative: true,
        };
        let s = solve(&wb, 0, &m);
        assert!((s.objective - 20.0).abs() < 1e-9, "{s:?}");
        assert_eq!(value(&s, "A1"), 0.0);
        assert_eq!(value(&s, "B1"), 1.0);
        assert_eq!(value(&s, "C1"), 1.0);
    }

    #[test]
    fn infeasible_unbounded_nonlinear() {
        let wb = book(&[("A1", "0"), ("B1", "=A1"), ("C1", "=A1*A1")]);
        let base = Model {
            objective: (0, 1),
            goal: Goal::Max,
            variables: vec![range("A1")],
            constraints: vec![],
            non_negative: true,
        };
        assert_eq!(solve(&wb, 0, &base).status, Status::Unbounded);
        let mut m = base.clone();
        m.constraints = vec![con("A1", Relation::Le, 1.0), con("A1", Relation::Ge, 2.0)];
        assert_eq!(solve(&wb, 0, &m).status, Status::Infeasible);
        let mut m = base.clone();
        m.objective = (0, 2);
        assert_eq!(solve(&wb, 0, &m).status, Status::NotLinear);
        // Value of: reach 7 → A1 = 7.
        let mut m = base;
        m.goal = Goal::Value(7.0);
        let s = solve(&wb, 0, &m);
        assert_eq!(s.status, Status::Optimal);
        assert_eq!(value(&s, "A1"), 7.0);
    }
}
