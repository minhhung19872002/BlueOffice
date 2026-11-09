//! Solver: linear programs over worksheet formulas (simplex), with integer and binary
//! variables by branch and bound.
//!
//! The model is read from the formulas themselves: the objective and every constraint's left
//! side are evaluated on the formula engine with each variable cell moved by one, giving the
//! linear coefficients; a check at another point rejects formulas that are not linear. The
//! linear program is solved with a dense two-phase simplex (Bland's rule, so it cannot cycle);
//! integer variables are branched on depth first, pruned by the relaxation's bound.
//!
//! Other methods live in [`crate::solver_nl`] (smooth nonlinear models: projected
//! quasi-Newton steps inside an augmented Lagrangian, with multistart and branch and bound),
//! [`crate::solver_ga`] (seeded differential evolution with permutation genes for `dif`) and
//! [`crate::solver_report`] (result, sensitivity and limits tables).

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
    /// The `n` cells take the values 1..=n, each once (a permutation).
    Dif,
}

/// Solving method.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Method {
    /// Linear models: simplex (+ branch and bound).
    #[default]
    Simplex,
    /// Smooth nonlinear models: reduced-gradient style local search.
    Nonlinear,
    /// Any model with bounded variables: differential evolution.
    Evolutionary,
}

/// Method options; every field is optional (method defaults apply).
#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Options {
    /// Stop after this many milliseconds (not enforced on targets without a clock).
    pub max_time_ms: Option<u64>,
    /// Inner iterations (nonlinear) or generations (evolutionary).
    pub max_iterations: Option<u32>,
    /// Constraint precision (relative violation accepted as satisfied).
    pub precision: Option<f64>,
    /// Convergence tolerance.
    pub convergence: Option<f64>,
    /// Nonlinear: also start from random points and keep the best.
    pub multistart: bool,
    /// Number of extra random starts.
    pub starts: Option<u32>,
    /// Seed of the random generator (multistart / evolutionary).
    pub seed: Option<u64>,
    /// Evolutionary: population size.
    pub population: Option<u32>,
    /// Evolutionary: probability that a gene is redrawn at random.
    pub mutation: Option<f64>,
    /// Branch and bound: maximum subproblems.
    pub max_subproblems: Option<u32>,
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
    /// Solving method.
    #[serde(default)]
    pub method: Method,
    /// Method options.
    #[serde(default)]
    pub options: Options,
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
    /// The evolutionary method needs a lower and an upper bound on every variable.
    MissingBounds,
    /// A time, iteration or subproblem limit stopped the search (`values` holds the best
    /// feasible point found, or nothing).
    Limit,
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
    /// Sensitivity data for the reports (not serialised).
    #[serde(skip)]
    pub detail: Option<Detail>,
}

/// Sensitivity of one variable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VarSens {
    /// Linear: reduced cost; nonlinear: reduced gradient (objective change per unit).
    pub reduced: f64,
    /// Linear: the objective coefficient.
    pub coef: Option<f64>,
    /// Linear: how far the coefficient may rise / fall with the same optimal basis
    /// (`f64::INFINITY` = no limit).
    pub inc: f64,
    /// See `inc`.
    pub dec: f64,
}

/// Sensitivity of one constraint row (see [`user_rows`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowSens {
    /// Linear: shadow price; nonlinear: Lagrange multiplier — objective change per unit
    /// increase of the right side.
    pub price: f64,
    /// Linear: how far the right side may rise / fall with the same prices.
    pub inc: f64,
    /// See `inc`.
    pub dec: f64,
}

/// What the reports need beyond the values.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Detail {
    /// The method that produced the solution.
    pub method: Method,
    /// The model has integer, binary or `dif` constraints.
    pub integer: bool,
    /// Per variable (empty: no sensitivity data for this method / model).
    pub vars: Vec<VarSens>,
    /// Per constraint row of [`user_rows`].
    pub rows: Vec<RowSens>,
    /// Model evaluations (recalculations) used.
    pub evaluations: u64,
}

// ------------------------------------------------------------------ constraint rows

/// Right side of a constraint row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Side {
    /// A constant.
    Value(f64),
    /// A cell.
    Cell((u32, u32)),
}

/// One scalar constraint `lhs rel rhs` (a `<=`, `>=` or `=` constraint expanded cell by cell).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct UserRow {
    pub lhs: (u32, u32),
    pub rel: Relation,
    pub rhs: Side,
    /// Index of the constraint in `model.constraints`.
    pub con: usize,
}

/// The scalar rows of the model's `<=` / `>=` / `=` constraints, in model order (`None`
/// when a right side is missing).
pub(crate) fn user_rows(model: &Model) -> Option<Vec<UserRow>> {
    let mut out = Vec::new();
    for (ci, con) in model.constraints.iter().enumerate() {
        if matches!(con.relation, Relation::Int | Relation::Bin | Relation::Dif) {
            continue;
        }
        let rhs_cells = con.rhs.as_ref().map(cells_of);
        for (k, cell) in cells_of(&con.lhs).into_iter().enumerate() {
            let rhs = match (&rhs_cells, con.rhs_value) {
                (Some(rc), _) => Side::Cell(*rc.get(k).or_else(|| rc.first())?),
                (None, Some(v)) => Side::Value(v),
                (None, None) => return None,
            };
            out.push(UserRow {
                lhs: cell,
                rel: con.relation,
                rhs,
                con: ci,
            });
        }
    }
    Some(out)
}

/// Rounds away binary noise (15 significant digits).
pub(crate) fn clean(v: f64) -> f64 {
    format!("{v:.14e}").parse().unwrap_or(v)
}

// ------------------------------------------------------------------ linear model

/// An affine function `c · x + c0` of the variables.
#[derive(Debug, Clone)]
struct Affine {
    c: Vec<f64>,
    c0: f64,
}

pub(crate) fn number(v: &Value) -> Option<f64> {
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

/// The final tableau of an optimal simplex run, kept for sensitivity ranging.
struct Tableau {
    t: Vec<Vec<f64>>,
    /// Phase-2 reduced costs of `minimise −c · x`.
    obj: Vec<f64>,
    basis: Vec<usize>,
    width: usize,
    /// Columns that may enter the basis (structural, slack and surplus).
    allowed: usize,
    /// Per row: the column that started as that row's unit vector (its slack for `<=`,
    /// its artificial otherwise), so its tableau column is `B⁻¹ eᵢ`.
    unit: Vec<usize>,
    /// Per row: the row was multiplied by −1 to make its right side non-negative.
    flip: Vec<bool>,
}

/// Maximise `c · x` subject to `rows`, `x ≥ 0`, by the two-phase simplex with Bland's rule.
fn simplex(c: &[f64], rows: &[Row]) -> Lp {
    simplex_tableau(c, rows).0
}

/// [`simplex`], also returning the optimal tableau.
#[allow(clippy::too_many_lines)]
fn simplex_tableau(c: &[f64], rows: &[Row]) -> (Lp, Option<Tableau>) {
    const EPS: f64 = 1e-9;
    let n = c.len();
    let m = rows.len();
    let flip: Vec<bool> = rows.iter().map(|r| r.b < 0.0).collect();
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
    let mut unit = vec![0usize; m];
    let (mut s, mut a) = (n, n + slacks);
    for (i, r) in rows.iter().enumerate() {
        t[i][..n].copy_from_slice(&r.a);
        t[i][width] = r.b;
        match r.rel {
            Relation::Le => {
                t[i][s] = 1.0;
                basis[i] = s;
                unit[i] = s;
                s += 1;
            }
            Relation::Ge => {
                t[i][s] = -1.0;
                s += 1;
                t[i][a] = 1.0;
                basis[i] = a;
                unit[i] = a;
                a += 1;
            }
            _ => {
                t[i][a] = 1.0;
                basis[i] = a;
                unit[i] = a;
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
        return (Lp::Infeasible, None);
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
        return (Lp::Unbounded, None);
    }
    let mut x = vec![0.0; n];
    for i in 0..m {
        if basis[i] < n {
            x[basis[i]] = t[i][width];
        }
    }
    let value = c.iter().zip(&x).map(|(a, b)| a * b).sum();
    let tab = Tableau {
        t,
        obj,
        basis,
        width,
        allowed: n + slacks,
        unit,
        flip,
    };
    (Lp::Optimal(x, value), Some(tab))
}

/// `(value, Δlo, Δhi)`: a sensitivity value and how far its parameter may move.
type Range = (f64, f64, f64);

/// Ranging on an optimal tableau (textbook post-optimal analysis). Per structural column
/// `j < n`: its reduced cost `d` in `minimise −c · x` and how far that cost may move with
/// the same basis. Per row: `∂(max c · x)/∂bᵢ` and how far `bᵢ` may move with the same basis.
fn ranging(tab: &Tableau, n: usize) -> (Vec<Range>, Vec<Range>) {
    const EPS: f64 = 1e-9;
    let inf = f64::INFINITY;
    let basic_row = |j: usize| tab.basis.iter().position(|&b| b == j);
    let cols = (0..n)
        .map(|j| match basic_row(j) {
            None => {
                // d' = d + Δ must stay ≥ 0.
                let d = tab.obj[j];
                (d, (-d).min(0.0), inf)
            }
            Some(r) => {
                // Changing the cost of a basic column by Δ moves every nonbasic reduced
                // cost d_k by −Δ·t[r][k]; each must stay ≥ 0.
                let (mut lo, mut hi) = (-inf, inf);
                for k in 0..tab.allowed {
                    if k == j || tab.basis.contains(&k) {
                        continue;
                    }
                    let v = tab.t[r][k];
                    let dk = tab.obj[k].max(0.0);
                    if v > EPS {
                        hi = hi.min(dk / v);
                    } else if v < -EPS {
                        lo = lo.max(dk / v);
                    }
                }
                (0.0, lo.min(0.0), hi.max(0.0))
            }
        })
        .collect();
    let rows = (0..tab.t.len())
        .map(|i| {
            // x_B(Δ) = x_B + Δ·B⁻¹eᵢ must stay ≥ 0.
            let col = tab.unit[i];
            let price = tab.obj[col];
            let (mut lo, mut hi) = (-inf, inf);
            for r in 0..tab.t.len() {
                let v = tab.t[r][col];
                let xb = tab.t[r][tab.width].max(0.0);
                if v > EPS {
                    lo = lo.max(-xb / v);
                } else if v < -EPS {
                    hi = hi.min(-xb / v);
                }
            }
            let (lo, hi) = (lo.min(0.0), hi.max(0.0));
            if tab.flip[i] {
                (-price, -hi, -lo)
            } else {
                (price, lo, hi)
            }
        })
        .collect();
    (cols, rows)
}

/// Integer structure of a linear model. Columns: `x⁺` of every variable, then `x⁻` of the
/// free ones (`x = x⁺ − x⁻`).
struct IntSpec {
    ints: Vec<bool>,
    groups: Vec<Vec<usize>>,
    minus: Vec<Option<usize>>,
    width: usize,
    max_nodes: usize,
}

impl IntSpec {
    fn value(&self, x: &[f64], j: usize) -> f64 {
        x[j] - self.minus[j].map_or(0.0, |k| x[k])
    }

    fn row(&self, j: usize, rel: Relation, b: f64) -> Row {
        let mut a = vec![0.0; self.width];
        a[j] = 1.0;
        if let Some(k) = self.minus[j] {
            a[k] = -1.0;
        }
        Row { a, rel, b }
    }
}

/// Two variables of one `dif` group with the same (rounded) value: `(i, k, value)`.
pub(crate) fn duplicate(
    groups: &[Vec<usize>],
    value: impl Fn(usize) -> f64,
) -> Option<(usize, usize, f64)> {
    for g in groups {
        for (a, &i) in g.iter().enumerate() {
            for &k in &g[a + 1..] {
                let (vi, vk) = (value(i).round(), value(k).round());
                if (vi - vk).abs() < 0.5 {
                    return Some((i, k, vi));
                }
            }
        }
    }
    None
}

/// Maximise with integer variables and `dif` groups by branch and bound (depth first).
/// A `dif` violation `xᵢ = xₖ = v` is split four ways: `xᵢ ≤ v−1`, `xᵢ ≥ v+1`,
/// `xᵢ = v ∧ xₖ ≤ v−1`, `xᵢ = v ∧ xₖ ≥ v+1`. Returns the result and whether the node limit
/// stopped the search.
fn branch_and_bound(c: &[f64], rows: &[Row], spec: &IntSpec) -> (Lp, bool) {
    let mut best: Option<(Vec<f64>, f64)> = None;
    let mut unbounded = false;
    let mut limit = false;
    let mut stack: Vec<Vec<Row>> = vec![Vec::new()];
    let mut nodes = 0;
    let n = spec.ints.len();
    while let Some(extra) = stack.pop() {
        nodes += 1;
        if nodes > spec.max_nodes {
            limit = true;
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
                let frac = (0..n).find(|&j| {
                    let v = spec.value(&x, j);
                    spec.ints[j] && (v - v.round()).abs() > 1e-6
                });
                if let Some(j) = frac {
                    let v = spec.value(&x, j);
                    let mut down = extra.clone();
                    down.push(spec.row(j, Relation::Le, v.floor()));
                    let mut up = extra;
                    up.push(spec.row(j, Relation::Ge, v.ceil()));
                    stack.push(up);
                    stack.push(down);
                    continue;
                }
                if let Some((i, k, v)) = duplicate(&spec.groups, |j| spec.value(&x, j)) {
                    let children = [
                        vec![spec.row(i, Relation::Le, v - 1.0)],
                        vec![spec.row(i, Relation::Ge, v + 1.0)],
                        vec![
                            spec.row(i, Relation::Eq, v),
                            spec.row(k, Relation::Le, v - 1.0),
                        ],
                        vec![
                            spec.row(i, Relation::Eq, v),
                            spec.row(k, Relation::Ge, v + 1.0),
                        ],
                    ];
                    for child in children.into_iter().rev() {
                        let mut e = extra.clone();
                        e.extend(child);
                        stack.push(e);
                    }
                    continue;
                }
                let x: Vec<f64> = x
                    .iter()
                    .enumerate()
                    .map(|(j, v)| {
                        if j < n && spec.ints[j] && spec.minus[j].is_none() {
                            v.round()
                        } else {
                            *v
                        }
                    })
                    .collect();
                best = Some((x, v));
            }
        }
    }
    let lp = match best {
        Some((x, v)) => Lp::Optimal(x, v),
        None if unbounded => Lp::Unbounded,
        None => Lp::Infeasible,
    };
    (lp, limit)
}

// ------------------------------------------------------------------ the sheet model

pub(crate) fn cells_of(r: &CellRange) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    for row in r.first.row..=r.last.row {
        for col in r.first.col..=r.last.col {
            out.push((row, col));
        }
    }
    out
}

/// A failed solve.
pub(crate) fn fail(status: Status) -> Solution {
    Solution {
        status,
        objective: 0.0,
        values: Vec::new(),
        detail: None,
    }
}

/// The starting values of the variable cells (numbers; anything else counts as 0).
pub(crate) fn start_values(wb: &Workbook, sheet: usize, vars: &[(u32, u32)]) -> Vec<f64> {
    let ws = &wb.sheets[sheet];
    vars.iter()
        .map(|k| match ws.cells.get(k).map(|c| &c.value) {
            Some(CellValue::Number(n)) => *n,
            _ => 0.0,
        })
        .collect()
}

/// Solve `model` on sheet `sheet` of `wb` with `model.method` (the workbook is not changed).
#[must_use]
pub fn solve(wb: &Workbook, sheet: usize, model: &Model) -> Solution {
    if sheet >= wb.sheets.len() {
        return fail(Status::Error);
    }
    match model.method {
        Method::Simplex => solve_linear(wb, sheet, model),
        Method::Nonlinear => crate::solver_nl::solve(wb, sheet, model),
        Method::Evolutionary => crate::solver_ga::solve(wb, sheet, model),
    }
}

#[allow(clippy::too_many_lines)]
fn solve_linear(wb: &Workbook, sheet: usize, model: &Model) -> Solution {
    let vars: Vec<(u32, u32)> = model.variables.iter().flat_map(cells_of).collect();
    if vars.is_empty() {
        return fail(Status::Error);
    }
    let base = start_values(wb, sheet, &vars);
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
    let mut minus = vec![None; n];
    let mut next = n;
    for (j, is_free) in free.iter().enumerate() {
        if *is_free {
            minus[j] = Some(next);
            next += 1;
        }
    }
    let expand = |a: &[f64]| -> Vec<f64> {
        let mut out = a.to_vec();
        for (j, is_free) in free.iter().enumerate() {
            if *is_free {
                out.push(-a[j]);
            }
        }
        out
    };
    let unit = |j: usize| {
        let mut a = vec![0.0; n];
        a[j] = 1.0;
        a
    };
    let mut rows: Vec<Row> = Vec::new();
    let mut ints = vec![false; n];
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for con in &model.constraints {
        match con.relation {
            Relation::Int | Relation::Bin => {
                for cell in cells_of(&con.lhs) {
                    let Some(j) = var_index(cell) else { continue };
                    ints[j] = true;
                    if con.relation == Relation::Bin {
                        rows.push(Row {
                            a: expand(&unit(j)),
                            rel: Relation::Le,
                            b: 1.0,
                        });
                    }
                }
            }
            Relation::Dif => {
                // 1 ≤ xⱼ ≤ k, integer, and the valid cut Σ xⱼ = k(k+1)/2.
                let mut group = Vec::new();
                for cell in cells_of(&con.lhs) {
                    let Some(j) = var_index(cell) else {
                        return fail(Status::Error);
                    };
                    group.push(j);
                }
                let size = group.len() as f64;
                let mut sum = vec![0.0; n];
                for &j in &group {
                    ints[j] = true;
                    sum[j] = 1.0;
                    rows.push(Row {
                        a: expand(&unit(j)),
                        rel: Relation::Ge,
                        b: 1.0,
                    });
                    rows.push(Row {
                        a: expand(&unit(j)),
                        rel: Relation::Le,
                        b: size,
                    });
                }
                rows.push(Row {
                    a: expand(&sum),
                    rel: Relation::Eq,
                    b: size * (size + 1.0) / 2.0,
                });
                groups.push(group);
            }
            _ => {}
        }
    }
    let Some(urows) = user_rows(model) else {
        return fail(Status::Error);
    };
    let mut user_index = Vec::with_capacity(urows.len());
    for ur in &urows {
        let l = f(ur.lhs);
        let (rc, r0) = match ur.rhs {
            Side::Cell(cell) => {
                let r = f(cell);
                (r.c.clone(), r.c0)
            }
            Side::Value(v) => (vec![0.0; n], v),
        };
        // (l.c − r.c) · x  rel  r0 − l.c0
        let a: Vec<f64> = l.c.iter().zip(&rc).map(|(x, y)| x - y).collect();
        user_index.push(rows.len());
        rows.push(Row {
            a: expand(&a),
            rel: ur.rel,
            b: r0 - l.c0,
        });
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
    let integer = ints.iter().any(|i| *i);
    let spec = IntSpec {
        ints,
        groups,
        minus,
        width,
        max_nodes: model
            .options
            .max_subproblems
            .map_or(20_000, |m| m.max(1) as usize),
    };
    let (lp, tab, limit) = if integer {
        let (lp, limit) = branch_and_bound(&c, &rows, &spec);
        (lp, None, limit)
    } else {
        let (lp, tab) = simplex_tableau(&c, &rows);
        (lp, tab, false)
    };
    match lp {
        Lp::Infeasible if limit => fail(Status::Limit),
        Lp::Infeasible => fail(Status::Infeasible),
        Lp::Unbounded => fail(Status::Unbounded),
        Lp::Optimal(x, _) => {
            let values: Vec<((u32, u32), f64)> = vars
                .iter()
                .enumerate()
                .map(|(j, cell)| (*cell, clean(spec.value(&x, j))))
                .collect();
            let objective = obj.c0
                + obj
                    .c
                    .iter()
                    .zip(&values)
                    .map(|(a, (_, v))| a * v)
                    .sum::<f64>();
            let mut detail = Detail {
                method: Method::Simplex,
                integer,
                ..Detail::default()
            };
            if let (Some(tab), false) = (&tab, matches!(model.goal, Goal::Value(_))) {
                let (cols, rws) = ranging(tab, n);
                let max = model.goal == Goal::Max;
                detail.vars = cols
                    .iter()
                    .zip(&obj.c)
                    .map(|(&(d, lo, hi), &coef)| {
                        if max {
                            VarSens {
                                reduced: -d + 0.0,
                                coef: Some(coef),
                                inc: -lo + 0.0,
                                dec: hi,
                            }
                        } else {
                            VarSens {
                                reduced: d + 0.0,
                                coef: Some(coef),
                                inc: hi,
                                dec: -lo + 0.0,
                            }
                        }
                    })
                    .collect();
                detail.rows = user_index
                    .iter()
                    .map(|&i| {
                        let (p, lo, hi) = rws[i];
                        RowSens {
                            price: if max { p } else { -p } + 0.0,
                            inc: hi,
                            dec: -lo + 0.0,
                        }
                    })
                    .collect();
            }
            Solution {
                status: if limit {
                    Status::Limit
                } else {
                    Status::Optimal
                },
                objective: clean(objective),
                values,
                detail: Some(detail),
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
            method: Method::Simplex,
            options: Options::default(),
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
            method: Method::Simplex,
            options: Options::default(),
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
            method: Method::Simplex,
            options: Options::default(),
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
            method: Method::Simplex,
            options: Options::default(),
        };
        let s = solve(&wb, 0, &m);
        assert!((s.objective - 20.0).abs() < 1e-9, "{s:?}");
        assert_eq!(value(&s, "A1"), 0.0);
        assert_eq!(value(&s, "B1"), 1.0);
        assert_eq!(value(&s, "C1"), 1.0);
    }

    #[test]
    fn alldifferent_by_branch_and_bound() {
        // min 2a + b − c − 2d with (a, b, c, d) a permutation of 1..=4. By the rearrangement
        // inequality the largest coefficient takes the smallest value: (1, 2, 3, 4) gives
        // 2 + 2 − 3 − 8 = −7, and swapping any two values raises it (e.g. (2, 1, 3, 4) → −6,
        // (1, 2, 4, 3) → −6). The relaxation's optimum (1, 1, 4, 4) repeats values, so the
        // `dif` branching is exercised.
        let wb = book(&[
            ("A1", "0"),
            ("B1", "0"),
            ("C1", "0"),
            ("D1", "0"),
            ("E1", "=2*A1+B1-C1-2*D1"),
        ]);
        let m = Model {
            objective: (0, 4),
            goal: Goal::Min,
            variables: vec![range("A1:D1")],
            constraints: vec![Constraint {
                lhs: range("A1:D1"),
                relation: Relation::Dif,
                rhs_value: None,
                rhs: None,
            }],
            non_negative: true,
            method: Method::Simplex,
            options: Options::default(),
        };
        let s = solve(&wb, 0, &m);
        assert_eq!(s.status, Status::Optimal, "{s:?}");
        assert_eq!(s.objective, -7.0, "{s:?}");
        let got: Vec<f64> = ["A1", "B1", "C1", "D1"]
            .iter()
            .map(|a| value(&s, a))
            .collect();
        assert_eq!(got, vec![1.0, 2.0, 3.0, 4.0]);
        // The same model by the nonlinear method's branch and bound.
        let mut nl = m.clone();
        nl.method = Method::Nonlinear;
        let s = solve(&wb, 0, &nl);
        assert_eq!(s.status, Status::Optimal, "{s:?}");
        assert_eq!(s.objective, -7.0, "{s:?}");
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
            method: Method::Simplex,
            options: Options::default(),
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
