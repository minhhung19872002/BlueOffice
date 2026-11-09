//! Solver for smooth nonlinear models, on the recalculated worksheet.
//!
//! The model is evaluated by writing the variable cells into the formula engine and
//! recalculating; derivatives are central finite differences (one-sided, second order, at a
//! bound). Variable bounds (constraints whose left side is a variable cell and whose right side
//! is a constant) are kept as simple bounds and handled by projection; every other constraint
//! goes into an augmented Lagrangian (Powell–Hestenes–Rockafellar):
//!
//! ```text
//! L(x; λ, ρ) = f(x) + Σ_eq (λᵢ hᵢ + ρ/2 hᵢ²) + Σ_ineq (max(0, λᵢ + ρ gᵢ)² − λᵢ²) / (2ρ)
//! ```
//!
//! minimised over the box by a projected quasi-Newton method (BFGS on the free variables —
//! those not held at a bound by their gradient — with an Armijo backtracking search along the
//! projected path), which is the reduced-gradient idea of moving only the free variables. The
//! multipliers are updated `λ ← λ + ρ g` (clamped at 0 for inequalities) and `ρ` grows tenfold
//! when the violation does not shrink by ¾. Multistart repeats the local search from seeded
//! random points; integer, binary and `dif` constraints are handled by branch and bound on the
//! bounds. Sources: Nocedal & Wright, *Numerical Optimization* (2nd ed.), ch. 6 (BFGS),
//! ch. 16.7 (projected gradient), ch. 17.3 (augmented Lagrangian).

use bo_formula::Value;
use bo_model::sheet::Workbook;

use crate::formula::load;
use crate::solver::{
    cells_of, clean, duplicate, fail, number, start_values, user_rows, Detail, Goal, Method, Model,
    Options, Relation, RowSens, Side, Solution, Status, VarSens,
};
use crate::solver_ga::Rng;

/// Values beyond this magnitude count as "unbounded".
const HUGE: f64 = 1e15;

// ------------------------------------------------------------------ budget

/// Evaluation and wall-clock budget of one solve.
pub(crate) struct Budget {
    #[cfg(not(target_arch = "wasm32"))]
    start: std::time::Instant,
    max_ms: Option<u64>,
    pub evals: u64,
    max_evals: u64,
}

impl Budget {
    fn new(max_ms: Option<u64>) -> Budget {
        Budget {
            #[cfg(not(target_arch = "wasm32"))]
            start: std::time::Instant::now(),
            max_ms,
            evals: 0,
            max_evals: 2_000_000,
        }
    }

    /// The budget is used up.
    pub fn out(&self) -> bool {
        if self.evals >= self.max_evals {
            return true;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(ms) = self.max_ms {
            return self.start.elapsed().as_millis() >= u128::from(ms);
        }
        #[cfg(target_arch = "wasm32")]
        let _ = self.max_ms;
        false
    }
}

/// Method settings with defaults filled in.
#[derive(Debug, Clone)]
pub(crate) struct Settings {
    pub tol: f64,
    pub precision: f64,
    pub max_iter: u32,
    pub multistart: bool,
    pub starts: u32,
    pub seed: u64,
    pub max_sub: u32,
    pub population: usize,
    pub mutation: f64,
}

impl Settings {
    pub fn new(o: &Options, method: Method) -> Settings {
        let evolutionary = method == Method::Evolutionary;
        Settings {
            tol: o
                .convergence
                .filter(|v| v.is_finite() && *v > 0.0)
                .unwrap_or(if evolutionary { 1e-4 } else { 1e-6 })
                .clamp(1e-12, 0.5),
            precision: o
                .precision
                .filter(|v| v.is_finite() && *v > 0.0)
                .unwrap_or(1e-6)
                .clamp(1e-12, 0.5),
            max_iter: o
                .max_iterations
                .unwrap_or(if evolutionary { 2000 } else { 500 })
                .max(1),
            multistart: o.multistart,
            starts: o.starts.unwrap_or(10).clamp(1, 1000),
            seed: o.seed.unwrap_or(0),
            max_sub: o.max_subproblems.unwrap_or(5000).max(1),
            population: o.population.unwrap_or(50).clamp(4, 2000) as usize,
            mutation: o
                .mutation
                .filter(|v| v.is_finite())
                .unwrap_or(0.075)
                .clamp(0.0, 1.0),
        }
    }
}

// ------------------------------------------------------------------ the problem

/// A general constraint `lhs rel rhs` (anything that is not a simple bound).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Gen {
    lhs: (u32, u32),
    rhs: Side,
    rel: Relation,
    /// `1 + |rhs|` at the start: constraints are scaled by it.
    scale: f64,
}

/// How a constraint row of [`user_rows`] is held.
#[derive(Debug, Clone, Copy)]
pub(crate) enum RowKind {
    /// As general constraint `i`.
    Gen(usize),
    /// As a bound on variable `j`.
    Bound(usize, f64),
}

/// What is minimised.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Target {
    /// The model's objective (in minimisation form).
    Sheet,
    /// `sign · x_j` (limits report).
    Var(usize, f64),
}

/// The model on a private copy of the formula engine.
pub(crate) struct Problem {
    fw: bo_formula::Workbook,
    sheet: usize,
    pub vars: Vec<(u32, u32)>,
    objective: (u32, u32),
    goal: Goal,
    pub gens: Vec<Gen>,
    pub rows: Vec<RowKind>,
    pub lo: Vec<f64>,
    pub hi: Vec<f64>,
    pub ints: Vec<bool>,
    pub groups: Vec<Vec<usize>>,
    pub x0: Vec<f64>,
    pub budget: Budget,
    pub target: Target,
}

impl Problem {
    /// Builds the problem (`Err` with the status to report).
    pub fn new(wb: &Workbook, sheet: usize, model: &Model) -> Result<Problem, Status> {
        let vars: Vec<(u32, u32)> = model.variables.iter().flat_map(cells_of).collect();
        if vars.is_empty() || sheet >= wb.sheets.len() {
            return Err(Status::Error);
        }
        let n = vars.len();
        let x0 = start_values(wb, sheet, &vars);
        let mut fw = load(wb, None);
        fw.recalculate_all();
        let var_index = |cell: (u32, u32)| vars.iter().position(|v| *v == cell);
        let mut lo = vec![f64::NEG_INFINITY; n];
        let mut hi = vec![f64::INFINITY; n];
        let mut ints = vec![false; n];
        let mut groups = Vec::new();
        for con in &model.constraints {
            match con.relation {
                Relation::Int | Relation::Bin => {
                    for cell in cells_of(&con.lhs) {
                        let Some(j) = var_index(cell) else { continue };
                        ints[j] = true;
                        if con.relation == Relation::Bin {
                            lo[j] = lo[j].max(0.0);
                            hi[j] = hi[j].min(1.0);
                        }
                    }
                }
                Relation::Dif => {
                    let mut group = Vec::new();
                    for cell in cells_of(&con.lhs) {
                        let j = var_index(cell).ok_or(Status::Error)?;
                        group.push(j);
                    }
                    let size = group.len() as f64;
                    for &j in &group {
                        ints[j] = true;
                        lo[j] = lo[j].max(1.0);
                        hi[j] = hi[j].min(size);
                    }
                    groups.push(group);
                }
                _ => {}
            }
        }
        let urows = user_rows(model).ok_or(Status::Error)?;
        let read = |fw: &bo_formula::Workbook, cell: (u32, u32)| {
            number(&fw.value(sheet, cell.0, cell.1)).unwrap_or(0.0)
        };
        let mut gens = Vec::new();
        let mut rows = Vec::with_capacity(urows.len());
        for ur in &urows {
            if let (Some(j), Side::Value(v)) = (var_index(ur.lhs), ur.rhs) {
                match ur.rel {
                    Relation::Le => hi[j] = hi[j].min(v),
                    Relation::Ge => lo[j] = lo[j].max(v),
                    _ => {
                        lo[j] = lo[j].max(v);
                        hi[j] = hi[j].min(v);
                    }
                }
                rows.push(RowKind::Bound(j, v));
            } else {
                let r = match ur.rhs {
                    Side::Value(v) => v,
                    Side::Cell(c) => read(&fw, c),
                };
                rows.push(RowKind::Gen(gens.len()));
                gens.push(Gen {
                    lhs: ur.lhs,
                    rhs: ur.rhs,
                    rel: ur.rel,
                    scale: 1.0 + r.abs(),
                });
            }
        }
        if let Goal::Value(v) = model.goal {
            gens.push(Gen {
                lhs: model.objective,
                rhs: Side::Value(v),
                rel: Relation::Eq,
                scale: 1.0 + v.abs(),
            });
        }
        if model.non_negative {
            for l in &mut lo {
                if *l == f64::NEG_INFINITY {
                    *l = 0.0;
                }
            }
        }
        Ok(Problem {
            fw,
            sheet,
            vars,
            objective: model.objective,
            goal: model.goal,
            gens,
            rows,
            lo,
            hi,
            ints,
            groups,
            x0,
            budget: Budget::new(model.options.max_time_ms),
            target: Target::Sheet,
        })
    }

    /// Writes `x` into the variable cells and recalculates.
    pub fn set(&mut self, x: &[f64]) {
        for (&(r, c), &v) in self.vars.iter().zip(x) {
            self.fw.set_value(self.sheet, r, c, Value::Number(v));
        }
        self.fw.recalculate();
        self.budget.evals += 1;
    }

    /// The number in `cell` after [`Problem::set`].
    pub fn read(&self, cell: (u32, u32)) -> Option<f64> {
        number(&self.fw.value(self.sheet, cell.0, cell.1))
    }

    /// The objective cell at `x`.
    pub fn objective_at(&mut self, x: &[f64]) -> Option<f64> {
        self.set(x);
        self.read(self.objective)
    }

    /// Value of a constraint side after [`Problem::set`].
    pub fn side(&self, s: Side) -> Option<f64> {
        match s {
            Side::Value(v) => Some(v),
            Side::Cell(c) => self.read(c),
        }
    }

    /// `(f, g)` at `x`: the minimisation-form target and the scaled constraints (`gᵢ ≤ 0`
    /// for inequalities, `gᵢ = 0` for equalities). `None` when a cell gives no number.
    pub fn eval(&mut self, x: &[f64]) -> Option<(f64, Vec<f64>)> {
        self.set(x);
        let f = match self.target {
            Target::Sheet => match self.goal {
                Goal::Max => -self.read(self.objective)?,
                Goal::Min => self.read(self.objective)?,
                Goal::Value(_) => 0.0,
            },
            Target::Var(j, sign) => sign * x[j],
        };
        let mut g = Vec::with_capacity(self.gens.len());
        for c in &self.gens {
            let l = self.read(c.lhs)?;
            let r = self.side(c.rhs)?;
            g.push(
                match c.rel {
                    Relation::Ge => r - l,
                    _ => l - r,
                } / c.scale,
            );
        }
        if !f.is_finite() || g.iter().any(|v| !v.is_finite()) {
            return None;
        }
        Some((f, g))
    }

    fn is_eq(&self, i: usize) -> bool {
        self.gens[i].rel == Relation::Eq
    }

    /// Largest (scaled) violation of `g`.
    pub fn violation(&self, g: &[f64]) -> f64 {
        g.iter()
            .enumerate()
            .map(|(i, v)| if self.is_eq(i) { v.abs() } else { v.max(0.0) })
            .fold(0.0, f64::max)
    }

    fn merit(&self, f: f64, g: &[f64], lam: &[f64], rho: f64) -> f64 {
        let mut v = f;
        for (i, (&gi, &li)) in g.iter().zip(lam).enumerate() {
            if self.is_eq(i) {
                v += li * gi + 0.5 * rho * gi * gi;
            } else {
                let t = (li + rho * gi).max(0.0);
                v += (t * t - li * li) / (2.0 * rho);
            }
        }
        v
    }
}

fn clamp(x: &[f64], lo: &[f64], hi: &[f64]) -> Vec<f64> {
    x.iter()
        .zip(lo.iter().zip(hi))
        .map(|(v, (l, h))| v.max(*l).min(*h))
        .collect()
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

fn norm_inf(a: &[f64]) -> f64 {
    a.iter().fold(0.0, |m, v| m.max(v.abs()))
}

/// Finite-difference Jacobian of `f` (vector valued) at `x` with value `f0`: `jac[j]` is
/// `∂f/∂xⱼ`. Central differences inside the box, second-order one-sided ones at a bound;
/// fixed variables (`lo = hi`) get 0.
fn jacobian(
    f: &mut dyn FnMut(&[f64]) -> Option<Vec<f64>>,
    x: &[f64],
    f0: &[f64],
    lo: &[f64],
    hi: &[f64],
) -> Option<Vec<Vec<f64>>> {
    let mut jac = Vec::with_capacity(x.len());
    for j in 0..x.len() {
        if lo[j] >= hi[j] {
            jac.push(vec![0.0; f0.len()]);
            continue;
        }
        let h = 6e-6 * x[j].abs().max(1.0);
        let at = |d: f64| {
            let mut p = x.to_vec();
            p[j] += d;
            p
        };
        let col = if x[j] + h <= hi[j] && x[j] - h >= lo[j] {
            let up = f(&at(h))?;
            let dn = f(&at(-h))?;
            up.iter()
                .zip(&dn)
                .map(|(u, d)| (u - d) / (2.0 * h))
                .collect()
        } else if x[j] + 2.0 * h <= hi[j] {
            let a = f(&at(h))?;
            let b = f(&at(2.0 * h))?;
            (0..f0.len())
                .map(|i| (-3.0 * f0[i] + 4.0 * a[i] - b[i]) / (2.0 * h))
                .collect()
        } else if x[j] - 2.0 * h >= lo[j] {
            let a = f(&at(-h))?;
            let b = f(&at(-2.0 * h))?;
            (0..f0.len())
                .map(|i| (3.0 * f0[i] - 4.0 * a[i] + b[i]) / (2.0 * h))
                .collect()
        } else {
            vec![0.0; f0.len()]
        };
        jac.push(col);
    }
    Some(jac)
}

// ------------------------------------------------------------------ box-constrained search

struct Inner {
    x: Vec<f64>,
    limit: bool,
}

/// Minimises the augmented Lagrangian over `lo ≤ x ≤ hi` (projected BFGS).
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
fn box_min(
    p: &mut Problem,
    lam: &[f64],
    rho: f64,
    x0: &[f64],
    lo: &[f64],
    hi: &[f64],
    tol: f64,
    max_iter: u32,
) -> Result<Inner, Status> {
    let n = x0.len();
    let mut merit = |p: &mut Problem, x: &[f64]| -> Option<f64> {
        let (f, g) = p.eval(x)?;
        Some(p.merit(f, &g, lam, rho))
    };
    let grad = |p: &mut Problem,
                merit: &mut dyn FnMut(&mut Problem, &[f64]) -> Option<f64>,
                x: &[f64],
                v: f64|
     -> Option<Vec<f64>> {
        let mut f = |x: &[f64]| merit(p, x).map(|v| vec![v]);
        let jac = jacobian(&mut f, x, &[v], lo, hi)?;
        Some(jac.into_iter().map(|c| c[0]).collect())
    };
    let identity = || {
        let mut h = vec![vec![0.0; n]; n];
        for (i, row) in h.iter_mut().enumerate() {
            row[i] = 1.0;
        }
        h
    };
    let mut x = clamp(x0, lo, hi);
    let mut v = merit(p, &x).ok_or(Status::Error)?;
    let mut g = grad(p, &mut merit, &x, v).ok_or(Status::Error)?;
    let mut h = identity();
    let mut fresh = true;
    let mut free_prev: Vec<bool> = Vec::new();
    let mut stall = 0;
    // Length of a steepest-descent trial step: doubles while full steps are accepted.
    let mut trial_len = (0.1 * norm_inf(x0)).max(1.0);
    for _ in 0..max_iter {
        if p.budget.out() {
            return Ok(Inner { x, limit: true });
        }
        if v < -HUGE || norm_inf(&x) > HUGE {
            return Err(Status::Unbounded);
        }
        // Free variables: not held at a bound by the gradient.
        let free: Vec<bool> = (0..n)
            .map(|j| {
                lo[j] < hi[j] && !((x[j] <= lo[j] && g[j] > 0.0) || (x[j] >= hi[j] && g[j] < 0.0))
            })
            .collect();
        let pg = (0..n)
            .filter(|&j| free[j])
            .fold(0.0, |m: f64, j| m.max(g[j].abs()));
        // Scale-free test: the first-order change over the size of x, relative to the value.
        if pg * (1.0 + norm_inf(&x)) <= tol * (1.0 + v.abs()) {
            return Ok(Inner { x, limit: false });
        }
        if free != free_prev {
            h = identity();
            fresh = true;
            free_prev.clone_from(&free);
        }
        let direction = |h: &[Vec<f64>]| -> Vec<f64> {
            (0..n)
                .map(|j| {
                    if free[j] {
                        -(0..n)
                            .filter(|&k| free[k])
                            .map(|k| h[j][k] * g[k])
                            .sum::<f64>()
                    } else {
                        0.0
                    }
                })
                .collect()
        };
        let mut d = direction(&h);
        if dot(&g, &d) >= 0.0 {
            h = identity();
            fresh = true;
            d = direction(&h);
        }
        let mut alpha = if fresh { trial_len / pg } else { 1.0 };
        let mut accepted = None;
        for tries in 0..60 {
            let xn: Vec<f64> = clamp(
                &x.iter()
                    .zip(&d)
                    .map(|(a, b)| a + alpha * b)
                    .collect::<Vec<_>>(),
                lo,
                hi,
            );
            let dx: Vec<f64> = xn.iter().zip(&x).map(|(a, b)| a - b).collect();
            if norm_inf(&dx) <= 1e-15 * (1.0 + norm_inf(&x)) {
                break;
            }
            if let Some(vn) = merit(p, &xn) {
                if vn <= v + 1e-4 * dot(&g, &dx) {
                    if fresh {
                        let len = norm_inf(&dx);
                        trial_len = (if tries == 0 { 2.0 * len } else { len }).max(1e-10);
                    }
                    accepted = Some((xn, vn));
                    break;
                }
            }
            alpha *= 0.5;
        }
        let Some((xn, vn)) = accepted else {
            if fresh {
                // No descent along the steepest projected direction: stationary to working
                // precision.
                return Ok(Inner { x, limit: false });
            }
            h = identity();
            fresh = true;
            continue;
        };
        let gn = grad(p, &mut merit, &xn, vn).ok_or(Status::Error)?;
        let s: Vec<f64> = xn.iter().zip(&x).map(|(a, b)| a - b).collect();
        let y: Vec<f64> = gn.iter().zip(&g).map(|(a, b)| a - b).collect();
        let sy = dot(&s, &y);
        let yy = dot(&y, &y);
        if sy > 1e-12 * dot(&s, &s).sqrt() * yy.sqrt() && yy > 0.0 {
            if fresh {
                let scale = sy / yy;
                for (i, row) in h.iter_mut().enumerate() {
                    row[i] = scale;
                }
            }
            // H ← (I − ρ s yᵀ) H (I − ρ y sᵀ) + ρ s sᵀ, ρ = 1 / sᵀy.
            let r = 1.0 / sy;
            let hy: Vec<f64> = h.iter().map(|row| dot(row, &y)).collect();
            let yhy = dot(&y, &hy);
            for i in 0..n {
                for k in 0..n {
                    h[i][k] += -r * (hy[i] * s[k] + s[i] * hy[k]) + (r * r * yhy + r) * s[i] * s[k];
                }
            }
            fresh = false;
        }
        let small = (v - vn).abs() <= 1e-15 * (1.0 + v.abs())
            && norm_inf(&s) <= 1e-12 * (1.0 + norm_inf(&x));
        stall = if small { stall + 1 } else { 0 };
        x = xn;
        v = vn;
        g = gn;
        if stall >= 3 {
            return Ok(Inner { x, limit: false });
        }
    }
    Ok(Inner { x, limit: true })
}

// ------------------------------------------------------------------ local search

/// Result of a local search (or of branch and bound / evolution).
#[derive(Debug, Clone)]
pub(crate) struct Local {
    pub x: Vec<f64>,
    /// Minimisation-form target at `x`.
    pub f: f64,
    /// Multipliers of the scaled general constraints.
    pub lam: Vec<f64>,
    /// `Optimal`, `Limit`, `Infeasible`, `Unbounded` or `Error`.
    pub status: Status,
}

impl Local {
    fn failed(status: Status, n: usize) -> Local {
        Local {
            x: vec![0.0; n],
            f: f64::INFINITY,
            lam: Vec::new(),
            status,
        }
    }

    pub fn feasible(&self) -> bool {
        matches!(self.status, Status::Optimal | Status::Limit)
    }

    /// `self` is a better answer than `other`.
    pub fn better(&self, other: &Local) -> bool {
        match (self.feasible(), other.feasible()) {
            (true, false) => true,
            (true, true) => self.f < other.f - 1e-12 * (1.0 + other.f.abs()),
            _ => false,
        }
    }
}

/// Augmented-Lagrangian local search from `x0` within `lo..hi`.
pub(crate) fn local(p: &mut Problem, x0: &[f64], lo: &[f64], hi: &[f64], s: &Settings) -> Local {
    let n = x0.len();
    if lo.iter().zip(hi).any(|(l, h)| l > h) {
        return Local::failed(Status::Infeasible, n);
    }
    let m = p.gens.len();
    let mut lam = vec![0.0; m];
    let mut rho = 10.0;
    let mut x = clamp(x0, lo, hi);
    let mut eta = 0.1;
    let mut limit = false;
    let mut last: Option<(f64, f64)> = None;
    for _ in 0..if m == 0 { 1 } else { 60 } {
        match box_min(p, &lam, rho, &x, lo, hi, s.tol, s.max_iter) {
            Ok(inner) => {
                x = inner.x;
                limit |= inner.limit;
            }
            // A penalty too weak for the objective's curvature leaves the merit unbounded:
            // strengthen it and retry from the last iterate.
            Err(Status::Unbounded) if m > 0 && rho < 1e10 => {
                rho *= 10.0;
                continue;
            }
            Err(st) => return Local::failed(st, n),
        }
        let Some((f, g)) = p.eval(&x) else {
            return Local::failed(Status::Error, n);
        };
        let viol = p.violation(&g);
        last = Some((f, viol));
        if m == 0 {
            break;
        }
        if p.budget.out() {
            limit = true;
            break;
        }
        if viol > eta && rho < 1e10 {
            // Not feasible enough for this penalty: strengthen it, keep the multipliers
            // (Conn, Gould & Toint's rule; updating them from a poor point overshoots).
            rho = (rho * 10.0).min(1e10);
            continue;
        }
        let mut dl: f64 = 0.0;
        for (i, (l, gi)) in lam.iter_mut().zip(&g).enumerate() {
            let new = if p.gens[i].rel == Relation::Eq {
                *l + rho * gi
            } else {
                (*l + rho * gi).max(0.0)
            };
            dl = dl.max((new - *l).abs());
            *l = new;
        }
        if viol <= s.precision && dl <= s.tol.max(1e-9) * 10.0 * (1.0 + norm_inf(&lam)) {
            break;
        }
        eta = (eta * 0.25).max(s.precision * 0.1);
    }
    let Some((f, viol)) = last else {
        return Local::failed(Status::Error, n);
    };
    let status = if viol > s.precision * 10.0 {
        Status::Infeasible
    } else if limit {
        Status::Limit
    } else {
        Status::Optimal
    };
    Local { x, f, lam, status }
}

/// A random point in the box around `x0` (unbounded sides: ±10·max(1, |x0|)).
fn random_point(rng: &mut Rng, x0: &[f64], lo: &[f64], hi: &[f64]) -> Vec<f64> {
    x0.iter()
        .enumerate()
        .map(|(j, &c)| {
            let w = 10.0 * c.abs().max(1.0);
            let a = if lo[j].is_finite() { lo[j] } else { c - w };
            let b = if hi[j].is_finite() { hi[j] } else { c + w };
            a + rng.uniform() * (b - a)
        })
        .collect()
}

/// [`local`] from `x0`, then (with multistart) from seeded random points; the best answer.
pub(crate) fn relax(p: &mut Problem, x0: &[f64], lo: &[f64], hi: &[f64], s: &Settings) -> Local {
    let mut best = local(p, x0, lo, hi, s);
    if s.multistart
        && matches!(
            best.status,
            Status::Optimal | Status::Limit | Status::Infeasible
        )
    {
        let mut rng = Rng::new(s.seed);
        for _ in 0..s.starts {
            if p.budget.out() {
                break;
            }
            let start = random_point(&mut rng, x0, lo, hi);
            let r = local(p, &start, lo, hi, s);
            if r.better(&best) {
                best = r;
            }
        }
    }
    best
}

/// Branch and bound on the bounds for integer, binary and `dif` variables.
pub(crate) fn branch(p: &mut Problem, s: &Settings) -> Local {
    let n = p.vars.len();
    let mut stack = vec![(p.lo.clone(), p.hi.clone(), p.x0.clone())];
    let mut best: Option<Local> = None;
    let mut nodes = 0;
    let mut limit = false;
    let mut unbounded = false;
    while let Some((lo, hi, start)) = stack.pop() {
        nodes += 1;
        if nodes > s.max_sub || p.budget.out() {
            limit = true;
            break;
        }
        let r = relax(p, &start, &lo, &hi, s);
        match r.status {
            Status::Unbounded => {
                unbounded = true;
                continue;
            }
            Status::Error => return r,
            Status::Optimal | Status::Limit => {}
            _ => continue,
        }
        if let Some(b) = &best {
            if r.f >= b.f - 1e-9 * (1.0 + b.f.abs()) {
                continue;
            }
        }
        let ints = &p.ints;
        if let Some(j) = (0..n).find(|&j| ints[j] && (r.x[j] - r.x[j].round()).abs() > 1e-6) {
            let mut down_hi = hi.clone();
            down_hi[j] = r.x[j].floor();
            let mut up_lo = lo.clone();
            up_lo[j] = r.x[j].ceil();
            let down = (lo.clone(), down_hi, r.x.clone());
            let up = (up_lo, hi.clone(), r.x.clone());
            // Explore the nearer side first.
            if r.x[j] - r.x[j].floor() < 0.5 {
                stack.push(up);
                stack.push(down);
            } else {
                stack.push(down);
                stack.push(up);
            }
            continue;
        }
        let xr: Vec<f64> =
            r.x.iter()
                .zip(ints)
                .map(|(v, i)| if *i { v.round() } else { *v })
                .collect();
        if let Some((i, k, v)) = duplicate(&p.groups, |j| xr[j]) {
            let mut kids = Vec::new();
            let (mut a, mut b) = (lo.clone(), hi.clone());
            b[i] = v - 1.0;
            kids.push((a.clone(), b.clone()));
            b.clone_from(&hi);
            a[i] = v + 1.0;
            kids.push((a.clone(), b.clone()));
            a.clone_from(&lo);
            a[i] = v;
            b[i] = v;
            b[k] = v - 1.0;
            kids.push((a.clone(), b.clone()));
            b[k] = hi[k];
            a[k] = v + 1.0;
            kids.push((a, b));
            for (a, b) in kids.into_iter().rev() {
                if a.iter().zip(&b).all(|(l, h)| l <= h) {
                    stack.push((a, b, xr.clone()));
                }
            }
            continue;
        }
        let Some((f, g)) = p.eval(&xr) else { continue };
        if p.violation(&g) <= s.precision * 10.0 && best.as_ref().is_none_or(|b| f < b.f) {
            best = Some(Local {
                x: xr,
                f,
                lam: r.lam,
                status: Status::Optimal,
            });
        }
    }
    match best {
        Some(mut b) => {
            if limit {
                b.status = Status::Limit;
            }
            b
        }
        None if limit => Local::failed(Status::Limit, n),
        None if unbounded => Local::failed(Status::Unbounded, n),
        None => Local::failed(Status::Infeasible, n),
    }
}

// ------------------------------------------------------------------ answers

/// Builds the [`Solution`] of a finished search (sensitivity for smooth continuous models).
pub(crate) fn finish(p: &mut Problem, r: &Local, method: Method, sensitivity: bool) -> Solution {
    if !r.feasible() {
        return fail(r.status);
    }
    let Some(objective) = p.objective_at(&r.x) else {
        return fail(Status::Error);
    };
    let integer = p.ints.iter().any(|i| *i);
    let mut detail = Detail {
        method,
        integer,
        ..Detail::default()
    };
    if sensitivity && !integer && !matches!(p.goal, Goal::Value(_)) {
        if let Some((vars, rows)) = multipliers(p, &r.x, &r.lam) {
            detail.vars = vars;
            detail.rows = rows;
        }
    }
    detail.evaluations = p.budget.evals;
    Solution {
        status: r.status,
        objective: clean(objective),
        values: p
            .vars
            .iter()
            .zip(&r.x)
            .map(|(c, v)| (*c, clean(*v)))
            .collect(),
        detail: Some(detail),
    }
}

/// Reduced gradients and Lagrange multipliers at `x`, in objective units: the reduced
/// gradient is `∂obj/∂xⱼ − Σ μᵢ ∂cᵢ/∂xⱼ`, `μᵢ` the change of the objective per unit
/// increase of constraint `i`'s right side; a bound's multiplier is the reduced gradient of
/// its variable when the bound is active.
fn multipliers(p: &mut Problem, x: &[f64], lam: &[f64]) -> Option<(Vec<VarSens>, Vec<RowSens>)> {
    let (f0, g0) = p.eval(x)?;
    let mut base = vec![f0];
    base.extend(&g0);
    let (lo, hi) = (p.lo.clone(), p.hi.clone());
    let jac = {
        let mut f = |x: &[f64]| {
            p.eval(x).map(|(f, g)| {
                let mut v = vec![f];
                v.extend(g);
                v
            })
        };
        jacobian(&mut f, x, &base, &lo, &hi)?
    };
    let max = p.goal == Goal::Max;
    let reduced: Vec<f64> = jac
        .iter()
        .map(|col| {
            let rg = col[0] + lam.iter().zip(&col[1..]).map(|(l, d)| l * d).sum::<f64>();
            (if max { -rg } else { rg }) + 0.0
        })
        .collect();
    let nan = f64::NAN;
    let vars = reduced
        .iter()
        .map(|&r| VarSens {
            reduced: r,
            coef: None,
            inc: nan,
            dec: nan,
        })
        .collect();
    let rows = p
        .rows
        .iter()
        .map(|k| {
            let price = match *k {
                RowKind::Gen(i) => {
                    let l = lam.get(i).copied().unwrap_or(0.0) / p.gens[i].scale;
                    let sign = if max { 1.0 } else { -1.0 };
                    let rel = if p.gens[i].rel == Relation::Ge {
                        -1.0
                    } else {
                        1.0
                    };
                    sign * rel * l
                }
                RowKind::Bound(j, v) => {
                    if (x[j] - v).abs() <= 1e-6 * (1.0 + v.abs()) {
                        reduced[j]
                    } else {
                        0.0
                    }
                }
            };
            RowSens {
                price: price + 0.0,
                inc: nan,
                dec: nan,
            }
        })
        .collect();
    Some((vars, rows))
}

/// Solves `model` with the nonlinear method.
pub(crate) fn solve(wb: &Workbook, sheet: usize, model: &Model) -> Solution {
    let mut p = match Problem::new(wb, sheet, model) {
        Ok(p) => p,
        Err(s) => return fail(s),
    };
    let s = Settings::new(&model.options, Method::Nonlinear);
    let r = if p.ints.iter().any(|i| *i) {
        branch(&mut p, &s)
    } else {
        let (x0, lo, hi) = (p.x0.clone(), p.lo.clone(), p.hi.clone());
        relax(&mut p, &x0, &lo, &hi, &s)
    };
    finish(&mut p, &r, Method::Nonlinear, true)
}

/// The lowest (`sign = 1`) or highest (`sign = −1`) value variable `j` can take with the
/// other variables held at `x`, and the objective there (limits report). `None`: unbounded
/// or not reachable.
pub(crate) fn limit(p: &mut Problem, x: &[f64], j: usize, sign: f64) -> Option<(f64, f64)> {
    let s = Settings::new(&Options::default(), Method::Nonlinear);
    let mut lo = x.to_vec();
    let mut hi = x.to_vec();
    lo[j] = p.lo[j];
    hi[j] = p.hi[j];
    p.target = Target::Var(j, sign);
    let r = local(p, x, &lo, &hi, &s);
    p.target = Target::Sheet;
    if !r.feasible() || r.x[j].abs() > HUGE / 10.0 {
        return None;
    }
    let obj = p.objective_at(&r.x)?;
    Some((clean(r.x[j]), clean(obj)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::{Cell, CellRange, CellRef, CellValue, Worksheet};

    use crate::solver::Constraint;

    pub(crate) fn book(cells: &[(&str, &str)]) -> Workbook {
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

    fn model(objective: &str, goal: Goal, vars: &str, constraints: Vec<Constraint>) -> Model {
        let o = CellRef::parse(objective).unwrap();
        Model {
            objective: (o.row, o.col),
            goal,
            variables: vec![range(vars)],
            constraints,
            non_negative: false,
            method: Method::Nonlinear,
            options: Options::default(),
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
    fn unconstrained_quadratic() {
        // (x − 3)² + (y + 1)² has its only minimum at (3, −1), value 0.
        let wb = book(&[("A1", "0"), ("B1", "0"), ("C1", "=(A1-3)^2+(B1+1)^2")]);
        let s = solve(&wb, 0, &model("C1", Goal::Min, "A1:B1", vec![]));
        assert_eq!(s.status, Status::Optimal, "{s:?}");
        assert!((value(&s, "A1") - 3.0).abs() < 1e-5, "{s:?}");
        assert!((value(&s, "B1") + 1.0).abs() < 1e-5, "{s:?}");
        assert!(s.objective.abs() < 1e-9);
    }

    #[test]
    fn rosenbrock_from_the_classic_start() {
        // 100 (y − x²)² + (1 − x)² from (−1.2, 1): the minimum is (1, 1), value 0.
        let wb = book(&[
            ("A1", "-1.2"),
            ("B1", "1"),
            ("C1", "=100*(B1-A1^2)^2+(1-A1)^2"),
        ]);
        let s = solve(&wb, 0, &model("C1", Goal::Min, "A1:B1", vec![]));
        assert_eq!(s.status, Status::Optimal, "{s:?}");
        assert!((value(&s, "A1") - 1.0).abs() < 1e-3, "{s:?}");
        assert!((value(&s, "B1") - 1.0).abs() < 2e-3, "{s:?}");
        assert!(s.objective < 1e-6, "{s:?}");
    }

    #[test]
    fn constrained_product_and_its_multiplier() {
        // max xy s.t. x + y ≤ 10, x, y ≥ 0. KKT: y = λ, x = λ, x + y = 10 → (5, 5), λ = 5:
        // raising the limit by one raises the optimum by about 5 (d(b²/4)/db = b/2 = 5).
        let wb = book(&[("A1", "1"), ("B1", "1"), ("C1", "=A1*B1"), ("C2", "=A1+B1")]);
        let mut m = model(
            "C1",
            Goal::Max,
            "A1:B1",
            vec![con("C2", Relation::Le, 10.0)],
        );
        m.non_negative = true;
        let s = solve(&wb, 0, &m);
        assert_eq!(s.status, Status::Optimal, "{s:?}");
        assert!((value(&s, "A1") - 5.0).abs() < 1e-4, "{s:?}");
        assert!((value(&s, "B1") - 5.0).abs() < 1e-4, "{s:?}");
        assert!((s.objective - 25.0).abs() < 1e-3, "{s:?}");
        let d = s.detail.unwrap();
        assert!((d.rows[0].price - 5.0).abs() < 1e-3, "{:?}", d.rows);
        // Interior variables: reduced gradient 0.
        assert!(
            d.vars.iter().all(|v| v.reduced.abs() < 1e-3),
            "{:?}",
            d.vars
        );
    }

    #[test]
    fn bound_multiplier_and_ge_constraint() {
        // min x² + y² s.t. x + y ≥ 2, y ≤ 0.5 → y = 0.5 (bound active), x = 1.5.
        // Multiplier of the ≥ row: ∂obj/∂x = 2x = 3 = μ·1 → μ = 3 (raising the right side
        // raises the minimum); bound on y: reduced gradient 2y − μ = 1 − 3 = −2.
        let wb = book(&[
            ("A1", "0"),
            ("B1", "0"),
            ("C1", "=A1^2+B1^2"),
            ("C2", "=A1+B1"),
        ]);
        let m = model(
            "C1",
            Goal::Min,
            "A1:B1",
            vec![con("C2", Relation::Ge, 2.0), con("B1", Relation::Le, 0.5)],
        );
        let s = solve(&wb, 0, &m);
        assert_eq!(s.status, Status::Optimal, "{s:?}");
        assert!((value(&s, "A1") - 1.5).abs() < 1e-4, "{s:?}");
        assert!((value(&s, "B1") - 0.5).abs() < 1e-6, "{s:?}");
        let d = s.detail.unwrap();
        assert!((d.rows[0].price - 3.0).abs() < 1e-3, "{:?}", d.rows);
        assert!((d.rows[1].price + 2.0).abs() < 1e-3, "{:?}", d.rows);
    }

    #[test]
    fn integer_branch_and_bound_on_a_curve() {
        // min (x − 2.6)² + (y − 1.4)² over integers → (3, 1).
        let wb = book(&[("A1", "0"), ("B1", "0"), ("C1", "=(A1-2.6)^2+(B1-1.4)^2")]);
        let mut m = model("C1", Goal::Min, "A1:B1", vec![]);
        m.constraints.push(Constraint {
            lhs: range("A1:B1"),
            relation: Relation::Int,
            rhs_value: None,
            rhs: None,
        });
        let s = solve(&wb, 0, &m);
        assert_eq!(s.status, Status::Optimal, "{s:?}");
        assert_eq!(value(&s, "A1"), 3.0);
        assert_eq!(value(&s, "B1"), 1.0);
    }

    #[test]
    fn multistart_escapes_a_local_minimum() {
        // f(x) = (x² − 4)² + x on [−3, 3]: local minima near x ≈ 1.97 (f ≈ 1.97) and
        // x ≈ −2.03 (f ≈ −2.03). From x = 1.5 the plain search stays on the right.
        let wb = book(&[("A1", "1.5"), ("B1", "=(A1^2-4)^2+A1")]);
        let mut m = model(
            "B1",
            Goal::Min,
            "A1",
            vec![con("A1", Relation::Ge, -3.0), con("A1", Relation::Le, 3.0)],
        );
        let s = solve(&wb, 0, &m);
        assert!(value(&s, "A1") > 0.0, "{s:?}");
        m.options.multistart = true;
        m.options.seed = Some(7);
        let s = solve(&wb, 0, &m);
        assert!(value(&s, "A1") < -1.9, "{s:?}");
        assert!(s.objective < -1.9, "{s:?}");
    }

    #[test]
    fn infeasible_and_unbounded() {
        let wb = book(&[("A1", "1"), ("B1", "=A1^2"), ("C1", "=A1^3")]);
        // x² ≤ −1 cannot hold.
        let m = model("B1", Goal::Min, "A1", vec![con("B1", Relation::Le, -1.0)]);
        assert_eq!(solve(&wb, 0, &m).status, Status::Infeasible);
        // max x³ with x free.
        let m = model("C1", Goal::Max, "A1", vec![]);
        assert_eq!(solve(&wb, 0, &m).status, Status::Unbounded);
        // Reach a value: x² = 9 from x = 1 → x = 3.
        let m = model("B1", Goal::Value(9.0), "A1", vec![]);
        let s = solve(&wb, 0, &m);
        assert_eq!(s.status, Status::Optimal, "{s:?}");
        assert!((value(&s, "A1") - 3.0).abs() < 1e-4, "{s:?}");
    }
}
