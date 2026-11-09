//! Evolutionary Solver: seeded differential evolution on the recalculated worksheet.
//!
//! Every variable needs a finite lower and upper bound (the `dif` cells get 1..=n). A
//! population of candidate points evolves by the classic DE/rand/1/bin step (Storn & Price,
//! 1997: `trial = a + F (b − c)` with binomial crossover), plus a random redraw of a gene with
//! probability `mutation`; integer genes are rounded. The cells of a `dif` group are encoded
//! as one permutation of 1..=n, recombined by order crossover (Davis, 1985) and mutated by
//! swapping two positions, so every candidate satisfies `dif` by construction. Constraints are
//! compared by Deb's feasibility rules (feasible beats infeasible; two infeasible points by
//! violation; two feasible points by objective). The search stops when the best point has not
//! improved by the convergence tolerance for 50 generations, or at the generation / time
//! limit; a continuous result is then polished by the nonlinear local search.

use bo_model::sheet::Workbook;

use crate::solver::{fail, Method, Model, Solution, Status};
use crate::solver_nl::{finish, local, Local, Problem, Settings};

/// SplitMix64 (Steele, Lea & Flood, 2014): a small seeded generator.
pub(crate) struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform in [0, 1).
    pub fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Uniform in 0..n (`n > 0`).
    pub fn below(&mut self, n: usize) -> usize {
        ((self.uniform() * n as f64) as usize).min(n.saturating_sub(1))
    }

    fn shuffle(&mut self, v: &mut [f64]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }
}

/// Fitness: `(violation, f)`, compared by Deb's rules.
#[derive(Debug, Clone, Copy)]
struct Key {
    viol: f64,
    f: f64,
}

impl Key {
    /// `self` is at least as good as `o` (feasible = violation ≤ `tol`).
    fn not_worse(self, o: Key, tol: f64) -> bool {
        match (self.viol <= tol, o.viol <= tol) {
            (true, true) => self.f <= o.f,
            (true, false) => true,
            (false, true) => false,
            (false, false) => self.viol <= o.viol,
        }
    }
}

fn key(p: &mut Problem, x: &[f64]) -> Key {
    match p.eval(x) {
        Some((f, g)) => Key {
            viol: p.violation(&g),
            f,
        },
        None => Key {
            viol: f64::INFINITY,
            f: f64::INFINITY,
        },
    }
}

/// Order crossover of two permutations: a slice of `a` in place, the rest in `b`'s order.
fn order_crossover(rng: &mut Rng, a: &[f64], b: &[f64]) -> Vec<f64> {
    let k = a.len();
    if k < 2 {
        return a.to_vec();
    }
    let (mut i, mut j) = (rng.below(k), rng.below(k));
    if i > j {
        std::mem::swap(&mut i, &mut j);
    }
    let mut out = vec![f64::NAN; k];
    out[i..=j].copy_from_slice(&a[i..=j]);
    let mut fill = b.iter().filter(|v| !a[i..=j].contains(v));
    for slot in &mut out {
        if slot.is_nan() {
            *slot = fill.next().copied().unwrap_or(0.0);
        }
    }
    out
}

/// Makes `x` a valid candidate: within bounds, integers rounded, each `dif` group a
/// permutation (ranked by value when it is not one already).
fn repair(p: &Problem, x: &mut [f64]) {
    for (j, v) in x.iter_mut().enumerate() {
        *v = v.max(p.lo[j]).min(p.hi[j]);
        if p.ints[j] {
            *v = v.round().max(p.lo[j].ceil()).min(p.hi[j].floor());
        }
    }
    for g in &p.groups {
        let mut vals: Vec<f64> = g.iter().map(|&j| x[j]).collect();
        let mut sorted = vals.clone();
        sorted.sort_by(f64::total_cmp);
        let is_perm = sorted
            .iter()
            .enumerate()
            .all(|(i, v)| (*v - (i + 1) as f64).abs() < 1e-9);
        if !is_perm {
            let mut order: Vec<usize> = (0..g.len()).collect();
            order.sort_by(|&a, &b| vals[a].total_cmp(&vals[b]));
            for (rank, &pos) in order.iter().enumerate() {
                vals[pos] = (rank + 1) as f64;
            }
            for (&j, v) in g.iter().zip(vals) {
                x[j] = v;
            }
        }
    }
}

/// Runs the evolution; the best point found.
#[allow(clippy::too_many_lines)]
fn evolve(p: &mut Problem, s: &Settings) -> Local {
    let n = p.vars.len();
    let mut rng = Rng::new(s.seed);
    let mut in_group = vec![false; n];
    for g in &p.groups {
        for &j in g {
            in_group[j] = true;
        }
    }
    let plain: Vec<usize> = (0..n).filter(|&j| !in_group[j]).collect();
    let size = s.population.max(4);
    let tol = s.precision * 10.0;
    // Initial population: the start point, then random points.
    let mut pop: Vec<Vec<f64>> = Vec::with_capacity(size);
    let mut first = p.x0.clone();
    repair(p, &mut first);
    pop.push(first);
    while pop.len() < size {
        let mut x = vec![0.0; n];
        for &j in &plain {
            x[j] = p.lo[j] + rng.uniform() * (p.hi[j] - p.lo[j]);
        }
        for g in &p.groups {
            let mut perm: Vec<f64> = (1..=g.len()).map(|v| v as f64).collect();
            rng.shuffle(&mut perm);
            for (&j, v) in g.iter().zip(perm) {
                x[j] = v;
            }
        }
        repair(p, &mut x);
        pop.push(x);
    }
    let mut keys: Vec<Key> = Vec::with_capacity(size);
    for x in &pop {
        let k = key(p, x);
        keys.push(k);
    }
    let best_of = |keys: &[Key]| {
        (0..keys.len())
            .reduce(|b, i| {
                if keys[i].not_worse(keys[b], tol) && !keys[b].not_worse(keys[i], tol) {
                    i
                } else {
                    b
                }
            })
            .unwrap_or(0)
    };
    let mut best = best_of(&keys);
    let mut stall = 0u32;
    let mut limit = true;
    for _ in 0..s.max_iter {
        if p.budget.out() {
            break;
        }
        let before = keys[best];
        let scale = 0.5 + 0.3 * rng.uniform();
        for i in 0..size {
            // Three distinct partners, none of them `i`.
            let mut pick = || loop {
                let k = rng.below(size);
                if k != i {
                    break k;
                }
            };
            let a = pick();
            let (mut b, mut c) = (pick(), pick());
            for _ in 0..16 {
                if b != a {
                    break;
                }
                b = pick();
            }
            for _ in 0..16 {
                if c != a && c != b {
                    break;
                }
                c = pick();
            }
            let mut trial = pop[i].clone();
            if !plain.is_empty() {
                let forced = plain[rng.below(plain.len())];
                for &j in &plain {
                    if j == forced || rng.uniform() < 0.9 {
                        trial[j] = pop[a][j] + scale * (pop[b][j] - pop[c][j]);
                    }
                    if rng.uniform() < s.mutation {
                        trial[j] = p.lo[j] + rng.uniform() * (p.hi[j] - p.lo[j]);
                    }
                    // Out of range: land between the parent and the bound.
                    if trial[j] < p.lo[j] {
                        trial[j] = p.lo[j] + rng.uniform() * (pop[i][j] - p.lo[j]);
                    } else if trial[j] > p.hi[j] {
                        trial[j] = p.hi[j] - rng.uniform() * (p.hi[j] - pop[i][j]);
                    }
                }
            }
            for g in &p.groups {
                let mine: Vec<f64> = g.iter().map(|&j| pop[i][j]).collect();
                let mut child = if rng.uniform() < 0.9 {
                    let other: Vec<f64> = g.iter().map(|&j| pop[a][j]).collect();
                    order_crossover(&mut rng, &other, &mine)
                } else {
                    mine
                };
                if child.len() > 1 && rng.uniform() < s.mutation.max(0.2) {
                    let (u, v) = (rng.below(child.len()), rng.below(child.len()));
                    child.swap(u, v);
                }
                for (&j, v) in g.iter().zip(child) {
                    trial[j] = v;
                }
            }
            repair(p, &mut trial);
            let k = key(p, &trial);
            if k.not_worse(keys[i], tol) {
                pop[i] = trial;
                keys[i] = k;
            }
        }
        best = best_of(&keys);
        let now = keys[best];
        let improved = if before.viol > tol {
            now.viol < before.viol - tol
        } else {
            now.f < before.f - s.tol * (1.0 + before.f.abs())
        };
        stall = if improved { 0 } else { stall + 1 };
        if stall >= 50 {
            limit = false;
            break;
        }
    }
    let k = keys[best];
    let mut out = Local {
        x: pop[best].clone(),
        f: k.f,
        lam: Vec::new(),
        status: if k.viol > tol {
            Status::Infeasible
        } else if limit {
            Status::Limit
        } else {
            Status::Optimal
        },
    };
    // Polish a continuous answer with the local search.
    if out.feasible() && !p.ints.iter().any(|i| *i) {
        let (lo, hi) = (p.lo.clone(), p.hi.clone());
        let r = local(
            p,
            &out.x,
            &lo,
            &hi,
            &Settings {
                tol: 1e-8,
                ..s.clone()
            },
        );
        if r.better(&out) {
            let status = out.status;
            out = r;
            out.lam = Vec::new();
            if status == Status::Limit {
                out.status = Status::Limit;
            }
        }
    }
    out
}

/// Solves `model` with the evolutionary method.
pub(crate) fn solve(wb: &Workbook, sheet: usize, model: &Model) -> Solution {
    let mut p = match Problem::new(wb, sheet, model) {
        Ok(p) => p,
        Err(s) => return fail(s),
    };
    if p.lo
        .iter()
        .zip(&p.hi)
        .any(|(l, h)| !l.is_finite() || !h.is_finite())
    {
        return fail(Status::MissingBounds);
    }
    if p.lo.iter().zip(&p.hi).any(|(l, h)| l > h) {
        return fail(Status::Infeasible);
    }
    let s = Settings::new(&model.options, Method::Evolutionary);
    let r = evolve(&mut p, &s);
    finish(&mut p, &r, Method::Evolutionary, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_model::sheet::{Cell, CellRange, CellRef, CellValue, Worksheet};

    use crate::solver::{Constraint, Goal, Options, Relation};

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

    fn con(lhs: &str, relation: Relation, rhs: Option<f64>) -> Constraint {
        Constraint {
            lhs: range(lhs),
            relation,
            rhs_value: rhs,
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
    fn generator_is_seeded() {
        let (mut a, mut b) = (Rng::new(3), Rng::new(3));
        for _ in 0..10 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        assert_ne!(Rng::new(3).next_u64(), Rng::new(4).next_u64());
        let mut r = Rng::new(1);
        assert!((0..1000).all(|_| (0.0..1.0).contains(&r.uniform())));
    }

    #[test]
    fn order_crossover_keeps_a_permutation() {
        let mut r = Rng::new(5);
        for _ in 0..50 {
            let mut c = order_crossover(&mut r, &[3.0, 1.0, 4.0, 2.0], &[1.0, 2.0, 3.0, 4.0]);
            c.sort_by(f64::total_cmp);
            assert_eq!(c, vec![1.0, 2.0, 3.0, 4.0]);
        }
    }

    #[test]
    fn needs_bounds() {
        let wb = book(&[("A1", "0"), ("B1", "=(A1-1)^2")]);
        let m = Model {
            objective: (0, 1),
            goal: Goal::Min,
            variables: vec![range("A1")],
            constraints: vec![],
            non_negative: true,
            method: Method::Evolutionary,
            options: Options::default(),
        };
        assert_eq!(solve(&wb, 0, &m).status, Status::MissingBounds);
    }

    #[test]
    fn bounded_nonsmooth_minimum() {
        // |x − 2| + |y + 1| + 1 on [−5, 5]²: minimum 1 at (2, −1) (not differentiable there).
        let wb = book(&[("A1", "4"), ("B1", "4"), ("C1", "=ABS(A1-2)+ABS(B1+1)+1")]);
        let m = Model {
            objective: (0, 2),
            goal: Goal::Min,
            variables: vec![range("A1:B1")],
            constraints: vec![
                con("A1:B1", Relation::Ge, Some(-5.0)),
                con("A1:B1", Relation::Le, Some(5.0)),
            ],
            non_negative: false,
            method: Method::Evolutionary,
            options: Options {
                seed: Some(11),
                ..Options::default()
            },
        };
        let s = solve(&wb, 0, &m);
        assert!(matches!(s.status, Status::Optimal | Status::Limit), "{s:?}");
        assert!((s.objective - 1.0).abs() < 1e-3, "{s:?}");
        assert!((value(&s, "A1") - 2.0).abs() < 1e-3, "{s:?}");
        assert!((value(&s, "B1") + 1.0).abs() < 1e-3, "{s:?}");
    }

    #[test]
    fn assignment_with_alldifferent() {
        // Worker i (row) takes job x_i (column 1..4); cost matrix (rows A..D):
        //        j1 j2 j3 j4
        //   A:    9  2  7  8
        //   B:    6  4  3  7
        //   C:    5  8  1  8
        //   D:    7  6  9  4
        // The 24 permutations (A, B, C, D), enumerated by hand in six blocks per job of A:
        //   A→j1 (9):  B2C3D4 18, B2C4D3 30, B3C2D4 24, B3C4D2 26, B4C2D3 33, B4C3D2 23
        //   A→j2 (2):  B1C3D4 13, B1C4D3 25, B3C1D4 14, B3C4D1 20, B4C1D3 23, B4C3D1 17
        //   A→j3 (7):  B1C2D4 25, B1C4D2 27, B2C1D4 20, B2C4D1 26, B4C1D2 25, B4C2D1 29
        //   A→j4 (8):  B1C2D3 31, B1C3D2 21, B2C1D3 26, B2C3D1 20, B3C1D2 22, B3C2D1 26
        // The unique optimum is (2, 1, 3, 4) = 2 + 6 + 1 + 4 = 13 (next best: 14).
        let wb = book(&[
            ("A1", "9"),
            ("B1", "2"),
            ("C1", "7"),
            ("D1", "8"),
            ("A2", "6"),
            ("B2", "4"),
            ("C2", "3"),
            ("D2", "7"),
            ("A3", "5"),
            ("B3", "8"),
            ("C3", "1"),
            ("D3", "8"),
            ("A4", "7"),
            ("B4", "6"),
            ("C4", "9"),
            ("D4", "4"),
            ("F1", "1"),
            ("F2", "2"),
            ("F3", "3"),
            ("F4", "4"),
            (
                "G1",
                "=INDEX(A1:D1,1,F1)+INDEX(A2:D2,1,F2)+INDEX(A3:D3,1,F3)+INDEX(A4:D4,1,F4)",
            ),
        ]);
        let m = Model {
            objective: (0, 6),
            goal: Goal::Min,
            variables: vec![range("F1:F4")],
            constraints: vec![con("F1:F4", Relation::Dif, None)],
            non_negative: true,
            method: Method::Evolutionary,
            options: Options {
                seed: Some(1),
                population: Some(20),
                ..Options::default()
            },
        };
        let s = solve(&wb, 0, &m);
        assert!(matches!(s.status, Status::Optimal | Status::Limit), "{s:?}");
        assert_eq!(s.objective, 13.0, "{s:?}");
        let got: Vec<f64> = ["F1", "F2", "F3", "F4"]
            .iter()
            .map(|a| value(&s, a))
            .collect();
        assert_eq!(got, vec![2.0, 1.0, 3.0, 4.0]);
    }
}
