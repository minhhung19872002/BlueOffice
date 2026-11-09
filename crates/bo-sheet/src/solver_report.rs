//! Solver reports: a result table (objective, variables and constraints before / after,
//! which constraints are at their limit and how much room the others have), a sensitivity
//! table (linear: reduced costs, objective coefficient ranges, shadow prices and right-side
//! ranges; nonlinear: reduced gradients and Lagrange multipliers) and a limits table (how
//! far each variable can move alone, the others held at the solution, and the objective
//! there). The engine writes each table on a new sheet.

use bo_model::sheet::{CellRef, CellValue, Workbook, Worksheet};
use serde::{Deserialize, Serialize};

use crate::solver::{user_rows, Detail, Method, Model, Relation, Side, Solution, Status};
use crate::solver_nl::{limit, Problem};

/// A report to produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReportKind {
    /// Values before / after and constraint status.
    Answer,
    /// Reduced costs / gradients and shadow prices / multipliers.
    Sensitivity,
    /// Range of each variable alone.
    Limits,
}

/// One cell of a report.
#[derive(Debug, Clone, PartialEq)]
pub enum RCell {
    /// Nothing.
    Empty,
    /// A label.
    Text(String),
    /// A number.
    Num(f64),
}

/// A report table.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// Which report.
    pub kind: ReportKind,
    /// Suggested sheet name.
    pub name: String,
    /// The rows (top-left at A1).
    pub rows: Vec<Vec<RCell>>,
    /// Rows to emphasise (title, section and column headings).
    pub strong: Vec<usize>,
}

/// Report wording (English, or Vietnamese with `vi`).
fn t(key: &str, vi: bool) -> &'static str {
    let (en, v) = match key {
        "answerName" => ("Solver result", "Kết quả tối ưu"),
        "sensName" => ("Solver sensitivity", "Độ nhạy tối ưu"),
        "limitsName" => ("Solver limits", "Giới hạn biến"),
        "answerTitle" => ("Optimisation result", "Kết quả bài toán tối ưu"),
        "sensTitle" => (
            "How the optimum responds to changes",
            "Độ nhạy của nghiệm tối ưu",
        ),
        "limitsTitle" => (
            "How far each variable can move on its own",
            "Mỗi biến có thể thay đổi riêng đến đâu",
        ),
        "sheet" => ("Model sheet", "Trang tính mô hình"),
        "method" => ("Method", "Phương pháp"),
        "outcome" => ("Outcome", "Kết quả"),
        "evaluations" => ("Recalculations", "Số lần tính lại"),
        "simplex" => ("Linear (simplex)", "Tuyến tính (đơn hình)"),
        "nonlinear" => ("Smooth nonlinear", "Phi tuyến trơn"),
        "evolutionary" => ("Evolutionary", "Tiến hoá"),
        "okSimplex" => (
            "Optimal solution; every constraint holds.",
            "Nghiệm tối ưu; mọi ràng buộc đều thoả.",
        ),
        "okNonlinear" => (
            "Converged to a local optimum; every constraint holds.",
            "Hội tụ tới một cực trị địa phương; mọi ràng buộc đều thoả.",
        ),
        "okEvolutionary" => (
            "Best point found once the population stopped improving.",
            "Điểm tốt nhất khi quần thể không còn cải thiện.",
        ),
        "stopped" => (
            "Stopped at a time, iteration or subproblem limit; best feasible point kept.",
            "Dừng do giới hạn thời gian, vòng lặp hoặc bài toán con; giữ điểm khả thi tốt nhất.",
        ),
        "objective" => ("Objective", "Mục tiêu"),
        "variables" => ("Variable cells", "Ô biến"),
        "constraints" => ("Constraints", "Ràng buộc"),
        "cell" => ("Cell", "Ô"),
        "label" => ("Label", "Nhãn"),
        "start" => ("Start value", "Giá trị ban đầu"),
        "result" => ("Result", "Kết quả"),
        "kind" => ("Kind", "Loại"),
        "value" => ("Value", "Giá trị"),
        "condition" => ("Condition", "Điều kiện"),
        "state" => ("State", "Trạng thái"),
        "room" => ("Room left", "Phần còn dư"),
        "continuous" => ("continuous", "liên tục"),
        "integer" => ("integer", "số nguyên"),
        "binary" => ("binary", "nhị phân"),
        "dif" => ("all different", "khác nhau từng đôi"),
        "atLimit" => ("at its limit", "chạm giới hạn"),
        "hasRoom" => ("has room", "còn dư"),
        "violated" => ("not met", "chưa thoả"),
        "holds" => ("holds", "thoả"),
        "reducedCost" => ("Reduced cost", "Chi phí rút gọn"),
        "coef" => ("Objective coefficient", "Hệ số mục tiêu"),
        "rise" => ("Can rise by", "Có thể tăng thêm"),
        "fall" => ("Can fall by", "Có thể giảm bớt"),
        "shadow" => ("Shadow price", "Giá bóng"),
        "rhs" => ("Right side", "Vế phải"),
        "reducedGradient" => ("Reduced gradient", "Gradient rút gọn"),
        "multiplier" => ("Lagrange multiplier", "Nhân tử Lagrange"),
        "noLimit" => ("no limit", "không giới hạn"),
        "unbounded" => ("unbounded", "không bị chặn"),
        "lowest" => ("Lowest value", "Giá trị thấp nhất"),
        "highest" => ("Highest value", "Giá trị cao nhất"),
        "objThere" => ("Objective there", "Mục tiêu tại đó"),
        "noSensInteger" => (
            "No sensitivity data: the model has integer, binary or all-different cells.",
            "Không có số liệu độ nhạy: mô hình có ô số nguyên, nhị phân hoặc khác nhau từng đôi.",
        ),
        "noSensMethod" => (
            "No sensitivity data for the evolutionary method.",
            "Phương pháp tiến hoá không cho số liệu độ nhạy.",
        ),
        "noSensValue" => (
            "No sensitivity data when the objective must reach a set value.",
            "Không có số liệu độ nhạy khi mục tiêu phải bằng một giá trị.",
        ),
        _ => ("", ""),
    };
    if vi {
        v
    } else {
        en
    }
}

fn text(s: &str) -> RCell {
    RCell::Text(s.to_owned())
}

fn num(v: f64) -> RCell {
    if v.is_finite() {
        RCell::Num(v + 0.0)
    } else {
        RCell::Empty
    }
}

fn a1(cell: (u32, u32)) -> String {
    CellRef::new(cell.0, cell.1).to_string()
}

/// The nearest text to the left of `cell` (else above it): what the model calls the cell.
fn label(ws: &Worksheet, cell: (u32, u32)) -> RCell {
    let text_at = |r: u32, c: u32| match ws.cells.get(&(r, c)).map(|x| &x.value) {
        Some(CellValue::Text(t)) if !t.trim().is_empty() => Some(t.clone()),
        _ => None,
    };
    let (r, c) = cell;
    (c.saturating_sub(50)..c)
        .rev()
        .find_map(|cc| text_at(r, cc))
        .or_else(|| {
            (r.saturating_sub(50)..r)
                .rev()
                .find_map(|rr| text_at(rr, c))
        })
        .map_or(RCell::Empty, RCell::Text)
}

fn fmt_num(v: f64) -> String {
    format!("{}", v + 0.0)
}

fn op(rel: Relation) -> &'static str {
    match rel {
        Relation::Le => "<=",
        Relation::Ge => ">=",
        _ => "=",
    }
}

/// Builder state shared by the three reports.
struct Ctx<'a> {
    ws: &'a Worksheet,
    vi: bool,
    rows: Vec<Vec<RCell>>,
    strong: Vec<usize>,
}

impl Ctx<'_> {
    fn push(&mut self, row: Vec<RCell>) {
        self.rows.push(row);
    }

    fn heading(&mut self, keys: &[&str]) {
        self.strong.push(self.rows.len());
        let vi = self.vi;
        self.rows
            .push(keys.iter().map(|k| text(t(k, vi))).collect());
    }

    fn cell_and_label(&self, cell: (u32, u32)) -> [RCell; 2] {
        [RCell::Text(a1(cell)), label(self.ws, cell)]
    }

    fn header(&mut self, sol: &Solution, title: &str, sheet_name: &str) {
        let vi = self.vi;
        self.strong.push(0);
        self.push(vec![text(t(title, vi))]);
        self.push(vec![text(t("sheet", vi)), text(sheet_name)]);
        let method = sol.detail.as_ref().map_or(Method::Simplex, |d| d.method);
        let (m, ok) = match method {
            Method::Simplex => ("simplex", "okSimplex"),
            Method::Nonlinear => ("nonlinear", "okNonlinear"),
            Method::Evolutionary => ("evolutionary", "okEvolutionary"),
        };
        self.push(vec![text(t("method", vi)), text(t(m, vi))]);
        let outcome = if sol.status == Status::Limit {
            "stopped"
        } else {
            ok
        };
        self.push(vec![text(t("outcome", vi)), text(t(outcome, vi))]);
        if let Some(d) = &sol.detail {
            if d.evaluations > 0 {
                self.push(vec![text(t("evaluations", vi)), num(d.evaluations as f64)]);
            }
        }
        self.push(Vec::new());
    }

    fn finish(self, kind: ReportKind, name: &str) -> Report {
        Report {
            kind,
            name: t(name, self.vi).to_owned(),
            rows: self.rows,
            strong: self.strong,
        }
    }
}

/// Kind label of each variable (`continuous` / `integer` / `binary` / `dif`).
fn kinds(model: &Model, vars: &[(u32, u32)]) -> Vec<&'static str> {
    let mut out = vec!["continuous"; vars.len()];
    for con in &model.constraints {
        let k = match con.relation {
            Relation::Int => "integer",
            Relation::Bin => "binary",
            Relation::Dif => "dif",
            _ => continue,
        };
        for cell in crate::solver::cells_of(&con.lhs) {
            if let Some(j) = vars.iter().position(|v| *v == cell) {
                out[j] = k;
            }
        }
    }
    out
}

/// Builds the requested reports for a successful `sol` of `model` on sheet `sheet` of `wb`
/// (the workbook as it was before the solution was entered). Labels are Vietnamese with
/// `vi`.
#[must_use]
pub fn build(
    wb: &Workbook,
    sheet: usize,
    model: &Model,
    sol: &Solution,
    kinds_wanted: &[ReportKind],
    vi: bool,
) -> Vec<Report> {
    if !matches!(sol.status, Status::Optimal | Status::Limit) || sol.values.is_empty() {
        return Vec::new();
    }
    let Ok(mut p) = Problem::new(wb, sheet, model) else {
        return Vec::new();
    };
    let Some(urows) = user_rows(model) else {
        return Vec::new();
    };
    if sol.values.len() != p.vars.len() {
        return Vec::new();
    }
    let ws = &wb.sheets[sheet];
    let x0 = p.x0.clone();
    let x1: Vec<f64> = sol.values.iter().map(|(_, v)| *v).collect();
    let mut out = Vec::new();
    let mut seen = Vec::new();
    for &kind in kinds_wanted {
        if seen.contains(&kind) {
            continue;
        }
        seen.push(kind);
        let mut c = Ctx {
            ws,
            vi,
            rows: Vec::new(),
            strong: Vec::new(),
        };
        match kind {
            ReportKind::Answer => answer(&mut c, &mut p, model, sol, &urows, &x0, &x1),
            ReportKind::Sensitivity => sensitivity(&mut c, &mut p, sol, &urows, &x1),
            ReportKind::Limits => limits(&mut c, &mut p, model, sol, &x1),
        }
        let name = match kind {
            ReportKind::Answer => "answerName",
            ReportKind::Sensitivity => "sensName",
            ReportKind::Limits => "limitsName",
        };
        out.push(c.finish(kind, name));
    }
    out
}

fn answer(
    c: &mut Ctx,
    p: &mut Problem,
    model: &Model,
    sol: &Solution,
    urows: &[crate::solver::UserRow],
    x0: &[f64],
    x1: &[f64],
) {
    let vi = c.vi;
    c.header(sol, "answerTitle", &c.ws.name.clone());
    // Objective.
    c.strong.push(c.rows.len());
    c.push(vec![text(t("objective", vi))]);
    c.heading(&["cell", "label", "start", "result"]);
    let before = p.objective_at(x0).map_or(RCell::Empty, num);
    let [a, l] = c.cell_and_label(model.objective);
    c.push(vec![a, l, before, num(sol.objective)]);
    c.push(Vec::new());
    // Variables.
    c.strong.push(c.rows.len());
    c.push(vec![text(t("variables", vi))]);
    c.heading(&["cell", "label", "start", "result", "kind"]);
    let kinds = kinds(model, &p.vars);
    for (j, &cell) in p.vars.clone().iter().enumerate() {
        let [a, l] = c.cell_and_label(cell);
        c.push(vec![a, l, num(x0[j]), num(x1[j]), text(t(kinds[j], vi))]);
    }
    c.push(Vec::new());
    // Constraints at the solution.
    c.strong.push(c.rows.len());
    c.push(vec![text(t("constraints", vi))]);
    c.heading(&["cell", "label", "value", "condition", "state", "room"]);
    p.set(x1);
    for ur in urows {
        let (Some(l), Some(r)) = (p.read(ur.lhs), p.side(ur.rhs)) else {
            continue;
        };
        let rhs_text = match ur.rhs {
            Side::Value(v) => fmt_num(v),
            Side::Cell(cell) => a1(cell),
        };
        let slack = match ur.rel {
            Relation::Le => r - l,
            Relation::Ge => l - r,
            _ => -(l - r).abs(),
        };
        let tol = 1e-6 * (1.0 + r.abs());
        let state = if slack < -tol * 10.0 {
            "violated"
        } else if slack <= tol {
            "atLimit"
        } else {
            "hasRoom"
        };
        let [a, lab] = c.cell_and_label(ur.lhs);
        c.push(vec![
            a,
            lab,
            num(crate::solver::clean(l)),
            RCell::Text(format!("{} {} {}", a1(ur.lhs), op(ur.rel), rhs_text)),
            text(t(state, vi)),
            num(if slack.abs() <= tol {
                0.0
            } else {
                crate::solver::clean(slack)
            }),
        ]);
    }
    for con in &model.constraints {
        let k = match con.relation {
            Relation::Int => "integer",
            Relation::Bin => "binary",
            Relation::Dif => "dif",
            _ => continue,
        };
        let first = (con.lhs.first.row, con.lhs.first.col);
        let last = (con.lhs.last.row, con.lhs.last.col);
        let addr = if first == last {
            a1(first)
        } else {
            format!("{}:{}", a1(first), a1(last))
        };
        let [_, lab] = c.cell_and_label(first);
        c.push(vec![
            RCell::Text(addr.clone()),
            lab,
            RCell::Empty,
            RCell::Text(format!("{addr} = {}", t(k, vi))),
            text(t("holds", vi)),
            RCell::Empty,
        ]);
    }
}

fn sensitivity(
    c: &mut Ctx,
    p: &mut Problem,
    sol: &Solution,
    urows: &[crate::solver::UserRow],
    x1: &[f64],
) {
    let vi = c.vi;
    c.header(sol, "sensTitle", &c.ws.name.clone());
    let empty = Detail::default();
    let d = sol.detail.as_ref().unwrap_or(&empty);
    if d.vars.len() != p.vars.len() || d.rows.len() != urows.len() {
        let why = if d.integer {
            "noSensInteger"
        } else if d.method == Method::Evolutionary {
            "noSensMethod"
        } else {
            "noSensValue"
        };
        c.push(vec![text(t(why, vi))]);
        return;
    }
    let linear = d.method == Method::Simplex;
    let limit_cell = |v: f64| {
        if v.is_infinite() {
            text(t("noLimit", vi))
        } else {
            num(crate::solver::clean(v))
        }
    };
    c.strong.push(c.rows.len());
    c.push(vec![text(t("variables", vi))]);
    if linear {
        c.heading(&[
            "cell",
            "label",
            "result",
            "reducedCost",
            "coef",
            "rise",
            "fall",
        ]);
    } else {
        c.heading(&["cell", "label", "result", "reducedGradient"]);
    }
    for (j, &cell) in p.vars.clone().iter().enumerate() {
        let s = d.vars[j];
        let [a, l] = c.cell_and_label(cell);
        let mut row = vec![a, l, num(x1[j]), num(crate::solver::clean(s.reduced))];
        if linear {
            row.push(s.coef.map_or(RCell::Empty, num));
            row.push(limit_cell(s.inc));
            row.push(limit_cell(s.dec));
        }
        c.push(row);
    }
    c.push(Vec::new());
    c.strong.push(c.rows.len());
    c.push(vec![text(t("constraints", vi))]);
    if linear {
        c.heading(&["cell", "label", "value", "shadow", "rhs", "rise", "fall"]);
    } else {
        c.heading(&["cell", "label", "value", "multiplier"]);
    }
    p.set(x1);
    for (ur, s) in urows.iter().zip(&d.rows) {
        let value = p
            .read(ur.lhs)
            .map_or(RCell::Empty, |v| num(crate::solver::clean(v)));
        let [a, l] = c.cell_and_label(ur.lhs);
        let mut row = vec![a, l, value, num(crate::solver::clean(s.price))];
        if linear {
            row.push(p.side(ur.rhs).map_or(RCell::Empty, num));
            row.push(limit_cell(s.inc));
            row.push(limit_cell(s.dec));
        }
        c.push(row);
    }
}

fn limits(c: &mut Ctx, p: &mut Problem, model: &Model, sol: &Solution, x1: &[f64]) {
    let vi = c.vi;
    c.header(sol, "limitsTitle", &c.ws.name.clone());
    c.strong.push(c.rows.len());
    c.push(vec![text(t("objective", vi))]);
    c.heading(&["cell", "label", "result"]);
    let [a, l] = c.cell_and_label(model.objective);
    c.push(vec![a, l, num(sol.objective)]);
    c.push(Vec::new());
    c.strong.push(c.rows.len());
    c.push(vec![text(t("variables", vi))]);
    c.heading(&[
        "cell", "label", "result", "lowest", "objThere", "highest", "objThere",
    ]);
    for (j, &cell) in p.vars.clone().iter().enumerate() {
        let [a, l] = c.cell_and_label(cell);
        let mut row = vec![a, l, num(x1[j])];
        for sign in [1.0, -1.0] {
            if let Some((v, o)) = limit(p, x1, j, sign) {
                row.push(num(v));
                row.push(num(o));
            } else {
                row.push(text(t("unbounded", vi)));
                row.push(RCell::Empty);
            }
        }
        c.push(row);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver::{solve, Constraint, Goal, Options};
    use bo_model::sheet::{Cell, CellRange};

    fn book(cells: &[(&str, &str)]) -> Workbook {
        let mut ws = Worksheet::new("Plan");
        for (a1, input) in cells {
            let r = CellRef::parse(a1).unwrap_or_default();
            if let Some(f) = input.strip_prefix('=') {
                ws.set_cell(r.row, r.col, Cell::default().with_formula(f));
            } else if let Ok(n) = input.parse::<f64>() {
                ws.set_a1(a1, CellValue::Number(n));
            } else {
                ws.set_a1(a1, CellValue::Text((*input).to_owned()));
            }
        }
        let mut wb = Workbook::default();
        wb.sheets.push(ws);
        wb
    }

    fn con(lhs: &str, relation: Relation, rhs: f64) -> Constraint {
        Constraint {
            lhs: CellRange::parse(lhs).unwrap(),
            relation,
            rhs_value: Some(rhs),
            rhs: None,
        }
    }

    /// The row whose first cell is `first`.
    fn find<'a>(r: &'a Report, first: &str) -> &'a [RCell] {
        r.rows
            .iter()
            .find(|row| row.first() == Some(&RCell::Text(first.to_owned())))
            .map_or_else(|| panic!("no row {first}: {:?}", r.rows), Vec::as_slice)
    }

    fn n(c: &RCell) -> f64 {
        match c {
            RCell::Num(v) => *v,
            other => panic!("not a number: {other:?}"),
        }
    }

    /// max 3x + 5y s.t. x ≤ 4, 2y ≤ 12, 3x + 2y ≤ 18 (A1 = x, B1 = y).
    fn classic() -> (Workbook, Model) {
        let wb = book(&[
            ("A1", "0"),
            ("B1", "0"),
            ("D1", "Profit"),
            ("E1", "=3*A1+5*B1"),
            ("D2", "Plant 2"),
            ("E2", "=2*B1"),
            ("D3", "Plant 3"),
            ("E3", "=3*A1+2*B1"),
        ]);
        let m = Model {
            objective: (0, 4),
            goal: Goal::Max,
            variables: vec![CellRange::parse("A1:B1").unwrap()],
            constraints: vec![
                con("A1", Relation::Le, 4.0),
                con("E2", Relation::Le, 12.0),
                con("E3", Relation::Le, 18.0),
            ],
            non_negative: true,
            method: Method::Simplex,
            options: Options::default(),
        };
        (wb, m)
    }

    #[test]
    fn linear_shadow_prices_match_the_hand_dual() {
        // Dual: min 4u + 12v + 18w s.t. u + 3w ≥ 3, 2v + 2w ≥ 5, u, v, w ≥ 0. At the primal
        // optimum (2, 6) the first row has slack 2, so u = 0; then 3w = 3 → w = 1 and
        // 2v + 2 = 5 → v = 1.5; dual value 12·1.5 + 18·1 = 36 = primal value.
        // Ranges (by hand, other data fixed): row 3's right side b keeps x = (b − 12)/3 in
        // [0, 4] for b ∈ [12, 24] → ±6; row 2's b keeps y = b/2 with x = (18 − b)/3 in [0, 4]
        // for b ∈ [6, 18] → ±6; row 1 has 2 to spare and no upper limit. The x coefficient
        // c keeps (2, 6) optimal while 0 ≤ c ≤ 7.5 (slope of 3x + 2y vs 2y) → +4.5 / −3; the y
        // coefficient while c ≥ 2 → no limit up, −3 down.
        let (wb, m) = classic();
        let s = solve(&wb, 0, &m);
        assert_eq!(s.status, Status::Optimal);
        let d = s.detail.clone().unwrap();
        let prices: Vec<f64> = d.rows.iter().map(|r| r.price).collect();
        for (got, want) in prices.iter().zip([0.0, 1.5, 1.0]) {
            assert!((got - want).abs() < 1e-9, "{prices:?}");
        }
        assert!((d.rows[0].dec - 2.0).abs() < 1e-9 && d.rows[0].inc.is_infinite());
        assert!((d.rows[1].inc - 6.0).abs() < 1e-9 && (d.rows[1].dec - 6.0).abs() < 1e-9);
        assert!((d.rows[2].inc - 6.0).abs() < 1e-9 && (d.rows[2].dec - 6.0).abs() < 1e-9);
        assert!((d.vars[0].inc - 4.5).abs() < 1e-9 && (d.vars[0].dec - 3.0).abs() < 1e-9);
        assert!(d.vars[1].inc.is_infinite() && (d.vars[1].dec - 3.0).abs() < 1e-9);
        assert!(d.vars.iter().all(|v| v.reduced == 0.0));
        let reports = build(&wb, 0, &m, &s, &[ReportKind::Sensitivity], false);
        let r = &reports[0];
        assert_eq!(r.name, "Solver sensitivity");
        let row = find(r, "E2");
        assert_eq!(row[1], RCell::Text("Plant 2".into()));
        assert_eq!(n(&row[2]), 12.0);
        assert_eq!(n(&row[3]), 1.5);
        assert_eq!(n(&row[4]), 12.0);
        let row = find(r, "A1");
        assert_eq!(row[6], RCell::Num(3.0));
        let row = find(r, "B1");
        assert_eq!(row[5], RCell::Text("no limit".into()));
    }

    #[test]
    fn minimise_reduced_cost_and_prices() {
        // min 2x + 3y s.t. x + y ≥ 4, x ≤ 10 → (4, 0), value 8. y is nonbasic: raising it by
        // one forces x down by one, costing 3 − 2 = 1 (reduced cost 1). Raising the ≥ limit
        // by one costs 2 (shadow price 2, by hand: x = b).
        let wb = book(&[
            ("A1", "0"),
            ("B1", "0"),
            ("C1", "=2*A1+3*B1"),
            ("C2", "=A1+B1"),
        ]);
        let m = Model {
            objective: (0, 2),
            goal: Goal::Min,
            variables: vec![CellRange::parse("A1:B1").unwrap()],
            constraints: vec![con("C2", Relation::Ge, 4.0), con("A1", Relation::Le, 10.0)],
            non_negative: true,
            method: Method::Simplex,
            options: Options::default(),
        };
        let s = solve(&wb, 0, &m);
        assert!((s.objective - 8.0).abs() < 1e-9);
        let d = s.detail.unwrap();
        assert!((d.vars[1].reduced - 1.0).abs() < 1e-9, "{:?}", d.vars);
        assert!((d.rows[0].price - 2.0).abs() < 1e-9, "{:?}", d.rows);
        assert_eq!(d.rows[1].price, 0.0);
        // y's cost may fall by 1 before y enters; no limit upwards.
        assert!((d.vars[1].dec - 1.0).abs() < 1e-9 && d.vars[1].inc.is_infinite());
    }

    #[test]
    fn answer_and_limits_reports() {
        let (wb, m) = classic();
        let s = solve(&wb, 0, &m);
        let reports = build(
            &wb,
            0,
            &m,
            &s,
            &[ReportKind::Answer, ReportKind::Limits],
            false,
        );
        assert_eq!(reports.len(), 2);
        let a = &reports[0];
        assert_eq!(a.name, "Solver result");
        let row = find(a, "E1");
        assert_eq!(row[1], RCell::Text("Profit".into()));
        assert_eq!((n(&row[2]), n(&row[3])), (0.0, 36.0));
        let row = find(a, "A1");
        assert_eq!((n(&row[2]), n(&row[3])), (0.0, 2.0));
        // Constraint rows follow the variables; A1 ≤ 4 has room 2, E3 ≤ 18 is at its limit.
        let cons: Vec<&Vec<RCell>> = a
            .rows
            .iter()
            .filter(|r| matches!(r.get(3), Some(RCell::Text(t)) if t.contains("<=")))
            .collect();
        assert_eq!(cons.len(), 3);
        assert_eq!(cons[0][4], RCell::Text("has room".into()));
        assert_eq!(n(&cons[0][5]), 2.0);
        assert_eq!(cons[2][4], RCell::Text("at its limit".into()));
        assert_eq!(n(&cons[2][5]), 0.0);
        // Limits, by hand with y = 6 fixed: x ∈ [0, 2] (3x + 12 ≤ 18) → objective 30 … 36;
        // with x = 2 fixed: y ∈ [0, 6] (2y ≤ 12 and 2y ≤ 12) → objective 6 … 36.
        let l = &reports[1];
        let row = find(l, "A1");
        assert!(
            n(&row[3]).abs() < 1e-5 && (n(&row[4]) - 30.0).abs() < 1e-4,
            "{row:?}"
        );
        assert!((n(&row[5]) - 2.0).abs() < 1e-5 && (n(&row[6]) - 36.0).abs() < 1e-4);
        let row = find(l, "B1");
        assert!(
            n(&row[3]).abs() < 1e-5 && (n(&row[4]) - 6.0).abs() < 1e-4,
            "{row:?}"
        );
        assert!((n(&row[5]) - 6.0).abs() < 1e-5);
        // Vietnamese labels.
        let vi = build(&wb, 0, &m, &s, &[ReportKind::Answer], true);
        assert_eq!(vi[0].name, "Kết quả tối ưu");
    }

    #[test]
    fn nonlinear_sensitivity_report_and_integer_note() {
        let wb = book(&[("A1", "1"), ("B1", "1"), ("C1", "=A1*B1"), ("C2", "=A1+B1")]);
        let mut m = Model {
            objective: (0, 2),
            goal: Goal::Max,
            variables: vec![CellRange::parse("A1:B1").unwrap()],
            constraints: vec![con("C2", Relation::Le, 10.0)],
            non_negative: true,
            method: Method::Nonlinear,
            options: Options::default(),
        };
        let s = solve(&wb, 0, &m);
        let r = &build(&wb, 0, &m, &s, &[ReportKind::Sensitivity], false)[0];
        assert!(r
            .rows
            .iter()
            .any(|row| row.get(3) == Some(&text("Lagrange multiplier"))));
        let row = find(r, "C2");
        assert!((n(&row[3]) - 5.0).abs() < 1e-3, "{row:?}");
        m.constraints.push(Constraint {
            lhs: CellRange::parse("A1:B1").unwrap(),
            relation: Relation::Int,
            rhs_value: None,
            rhs: None,
        });
        let s = solve(&wb, 0, &m);
        assert_eq!(s.objective, 25.0);
        let r = &build(&wb, 0, &m, &s, &[ReportKind::Sensitivity], false)[0];
        assert!(r.rows.iter().any(|row| matches!(row.first(),
            Some(RCell::Text(t)) if t.starts_with("No sensitivity data: the model has integer"))));
    }
}
