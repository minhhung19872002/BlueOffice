//! SHEET-PERF-002: sort, AutoFilter and pivot steps undo / redo without a copy of
//! the cells, and the formula engine is updated incrementally. On random
//! workbooks: undo gives back the identical model, redo the identical model after
//! the edit, and the incremental results equal a full reload of the formula engine.

use crate::sheet::SheetEngine;
use bo_model::sheet::Workbook;
use serde_json::{json, Value};

struct Lcg(u64);

impl Lcg {
    fn next(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        (self.0 >> 33) % n.max(1)
    }
}

fn run(e: &mut SheetEngine, v: &Value) -> Value {
    let s = e.exec_json(&v.to_string()).unwrap();
    serde_json::from_str(&s).unwrap()
}

fn rng(r0: u32, c0: u32, r1: u32, c1: u32) -> Value {
    json!({ "r0": r0, "c0": c0, "r1": r1, "c1": c1 })
}

fn model(e: &SheetEngine) -> Workbook {
    e.workbook().unwrap().clone()
}

/// The values a full reload of the formula engine gives for `wb`.
fn reloaded(wb: &Workbook) -> Workbook {
    let mut e = SheetEngine::new();
    e.load(wb.clone(), "x.xlsx").unwrap();
    model(&e)
}

const REGIONS: [&str; 5] = ["Bắc", "Nam", "Trung", "bắc", "Tây Nguyên"];

/// Sheet `Data`: a header and `n` records in A:D, formulas in E (relative,
/// absolute and to the row above), formulas outside the list in H; sheet `Other`
/// reads the list.
fn random_engine(g: &mut Lcg, n: u32) -> SheetEngine {
    let mut e = SheetEngine::new();
    e.new_workbook("Book1.xlsx").unwrap();
    e.rename_sheet(0, "Data").unwrap();
    e.add_sheet(Some("Other")).unwrap();
    let mut cells = Vec::new();
    for (c, h) in ["Vùng", "Năm", "Số", "Ghi chú", "Tính"].iter().enumerate() {
        cells.push((0, c as u32, (*h).to_owned()));
    }
    for r in 1..=n {
        let region = REGIONS[g.next(5) as usize].to_owned();
        cells.push((r, 0, region));
        if g.next(6) != 0 {
            cells.push((r, 1, (2015 + g.next(5)).to_string()));
        }
        cells.push((r, 2, (g.next(200) as i64 - 50).to_string()));
        match g.next(4) {
            0 => {}
            1 => cells.push((r, 3, "TRUE".into())),
            _ => cells.push((r, 3, format!("x{}", g.next(7)))),
        }
        let a1 = r + 1;
        let f = match g.next(5) {
            0 => format!("=C{a1}*2"),
            1 => format!("=A{a1}&\"-\"&B{a1}"),
            2 => format!("=C{}+1", a1 - 1),
            3 => format!("=C{a1}+$C$2"),
            _ => String::new(),
        };
        if !f.is_empty() {
            cells.push((r, 4, f));
        }
    }
    let last = n + 1;
    cells.push((0, 7, format!("=SUM(C2:C{last})")));
    cells.push((1, 7, format!("=SUBTOTAL(109,C2:C{last})")));
    cells.push((2, 7, "=INDEX(A:A,3)&C4".into()));
    cells.push((3, 7, format!("=COUNTIF(A2:A{last},\"Nam\")")));
    cells.push((4, 7, "=E3".into()));
    e.set_inputs(0, &cells).unwrap();
    e.set_inputs(
        1,
        &[
            (0, 0, "=Data!C2+Data!C3".into()),
            (1, 0, "=SUM(B3:F40)".into()),
            (6, 3, "old".into()),
            (7, 3, "=Data!A2".into()),
        ],
    )
    .unwrap();
    e
}

fn random_keys(g: &mut Lcg) -> Value {
    let mut keys = Vec::new();
    for _ in 0..=g.next(2) {
        keys.push(json!({ "col": g.next(5), "descending": g.next(2) == 1 }));
    }
    Value::Array(keys)
}

/// After the edit `cmd`: the incremental result equals a full reload; undo gives
/// back `before` exactly, redo the state after the edit.
fn check_round_trip(e: &mut SheetEngine, cmd: &Value, what: &str) {
    let before = model(e);
    run(e, cmd);
    let after = model(e);
    assert_eq!(reloaded(&after), after, "{what}: incremental ≠ full reload");
    assert!(e.undo().unwrap());
    let undone = model(e);
    if undone != before {
        let (a, b) = (format!("{undone:?}"), format!("{before:?}"));
        let at = a
            .bytes()
            .zip(b.bytes())
            .position(|(x, y)| x != y)
            .unwrap_or(0);
        let from = at.saturating_sub(300);
        panic!(
            "{what}: undo differs at {at}:\n now {}\n was {}",
            &a[from..(at + 300).min(a.len())],
            &b[from..(at + 300).min(b.len())]
        );
    }
    assert!(e.redo().unwrap());
    assert_eq!(model(e), after, "{what}: redo");
    assert!(e.undo().unwrap());
    assert_eq!(model(e), before, "{what}: second undo");
    assert!(e.redo().unwrap());
}

// source: ECMA-376 Part 1, §18.3.1.92 sortState; §18.17.2.3 relative references follow moved rows
#[test]
fn random_sorts_undo_redo_exactly_and_match_a_full_reload() {
    for seed in 0..16u64 {
        let mut g = Lcg(seed + 1);
        let n = 5 + g.next(40) as u32;
        let mut e = random_engine(&mut g, n);
        if g.next(2) == 1 {
            run(
                &mut e,
                &json!({ "cmd": "toggleFilter", "sheet": 0, "range": rng(0, 0, 0, 0) }),
            );
        }
        for step in 0..3 {
            let cmd = json!({ "cmd": "sort", "sheet": 0, "range": rng(0, 0, n, 4),
                "hasHeader": true, "keys": random_keys(&mut g) });
            check_round_trip(&mut e, &cmd, &format!("seed {seed} sort {step}"));
        }
    }
}

// source: ECMA-376 Part 1, §18.3.1.2 autoFilter, §18.17.7 SUBTOTAL (hidden rows)
#[test]
fn random_filters_undo_redo_exactly() {
    for seed in 0..8u64 {
        let mut g = Lcg(seed + 100);
        let n = 5 + g.next(30) as u32;
        let mut e = random_engine(&mut g, n);
        check_round_trip(
            &mut e,
            &json!({ "cmd": "toggleFilter", "sheet": 0, "range": rng(0, 0, 0, 0) }),
            "toggle",
        );
        let values: Vec<&str> = REGIONS.iter().copied().filter(|_| g.next(2) == 1).collect();
        check_round_trip(
            &mut e,
            &json!({ "cmd": "setFilter", "sheet": 0, "col": 0, "filter": { "values": values } }),
            &format!("seed {seed} filter"),
        );
        check_round_trip(
            &mut e,
            &json!({ "cmd": "showAllData", "sheet": 0 }),
            &format!("seed {seed} show all"),
        );
    }
}

// source: ECMA-376 Part 1, §18.10 pivot tables (pivotTableDefinition, pivotCacheDefinition)
#[test]
fn random_pivots_undo_redo_exactly() {
    for seed in 0..6u64 {
        let mut g = Lcg(seed + 500);
        let n = 5 + g.next(40) as u32;
        let mut e = random_engine(&mut g, n);
        let create = json!({ "cmd": "pivotCreate", "sheet": 0, "range": rng(0, 0, n, 3),
            "dest": { "sheet": 1, "row": 2, "col": 1 },
            "rows": [0], "cols": [1], "values": [{ "field": 2, "function": "sum" }] });
        check_round_trip(&mut e, &create, &format!("seed {seed} create"));
        // Change the source, then refresh.
        for _ in 0..4 {
            let r = 1 + g.next(u64::from(n)) as u32;
            e.set_input(0, r, 2, &g.next(900).to_string()).unwrap();
        }
        e.set_input(0, 1, 0, "Đông").unwrap();
        check_round_trip(
            &mut e,
            &json!({ "cmd": "pivotRefresh", "sheet": 1, "index": 0 }),
            &format!("seed {seed} refresh"),
        );
        // A second pivot on a new sheet (sheet count changes).
        let create2 = json!({ "cmd": "pivotCreate", "sheet": 0, "range": rng(0, 0, n, 3),
            "rows": [3], "values": [{ "field": 2, "function": "count" }] });
        check_round_trip(
            &mut e,
            &create2,
            &format!("seed {seed} create on a new sheet"),
        );
    }
}

#[test]
fn sort_undo_does_not_copy_the_cells() {
    let mut g = Lcg(7);
    let mut e = random_engine(&mut g, 30);
    run(
        &mut e,
        &json!({ "cmd": "sort", "sheet": 0, "range": rng(0, 0, 30, 4),
            "hasHeader": true, "keys": [{ "col": 2 }] }),
    );
    match e.undo.last() {
        Some(crate::sheet::UndoEntry::Light(step)) => assert!(step.holds_no_cells()),
        _ => panic!("sort should record a light undo step"),
    }
}

/// A refused pivot table changes nothing.
#[test]
fn failed_pivot_leaves_the_workbook_unchanged() {
    let mut g = Lcg(9);
    let mut e = random_engine(&mut g, 10);
    let before = model(&e);
    // Overlapping its own source: refused.
    let bad = json!({ "cmd": "pivotCreate", "sheet": 0, "range": rng(0, 0, 10, 3),
        "dest": { "sheet": 0, "row": 2, "col": 1 },
        "rows": [0], "values": [{ "field": 2, "function": "sum" }] });
    assert!(e.exec_json(&bad.to_string()).is_err());
    assert_eq!(model(&e), before);
}

/// The last undo step is a light one (no copy of the whole workbook).
fn assert_light(e: &SheetEngine, what: &str) {
    assert!(
        matches!(e.undo.last(), Some(crate::sheet::UndoEntry::Light(_))),
        "{what}: expected a light undo step"
    );
}

/// [`check_round_trip`] and the step recorded is a light one.
fn check_light(e: &mut SheetEngine, cmd: &Value, what: &str) {
    check_round_trip(e, cmd, what);
    assert_light(e, what);
}

fn col_name(c: u32) -> String {
    let a = bo_model::sheet::CellRef::new(0, c).to_string();
    a.trim_end_matches(|ch: char| ch.is_ascii_digit())
        .to_owned()
}

// source: ECMA-376 Part 1, §18.3.1.92 sortState/@columnSort (left-to-right sort)
#[test]
fn random_left_to_right_sorts_undo_redo_exactly() {
    for seed in 0..8u64 {
        let mut g = Lcg(seed + 900);
        let n = 5 + g.next(20) as u32;
        let mut e = random_engine(&mut g, n);
        // Sheet `Other` rows 21..23 × n columns, formulas reading across.
        let mut cells = Vec::new();
        for c in 0..n {
            cells.push((20, c, g.next(50).to_string()));
            cells.push((21, c, REGIONS[g.next(5) as usize].to_owned()));
            if g.next(3) == 0 {
                let at = bo_model::sheet::CellRef::new(20, c).to_string();
                cells.push((22, c, format!("={at}*3")));
            }
        }
        cells.push((25, 0, format!("=SUM(A21:{}21)", col_name(n - 1))));
        e.set_inputs(1, &cells).unwrap();
        let cmd = json!({ "cmd": "sort", "sheet": 1, "range": rng(20, 0, 22, n - 1),
            "hasHeader": false, "leftToRight": true,
            "keys": [{ "col": 21 }, { "col": 20, "descending": true }] });
        check_light(&mut e, &cmd, &format!("seed {seed} left to right"));
    }
}

// source: ECMA-376 Part 1, §18.3.1.85 sheetProtection (locked cells keep their content)
#[test]
fn sorts_with_a_protected_sheet_undo_lightly_and_refuse_locked_cells() {
    let mut g = Lcg(31);
    let mut e = random_engine(&mut g, 25);
    run(&mut e, &json!({ "cmd": "protectSheet", "sheet": 1 }));
    let sort = json!({ "cmd": "sort", "sheet": 0, "range": rng(0, 0, 25, 4),
        "hasHeader": true, "keys": [{ "col": 2 }] });
    check_light(&mut e, &sort, "protected other sheet");
    match e.undo.last() {
        Some(crate::sheet::UndoEntry::Light(step)) => assert!(step.holds_no_cells()),
        _ => panic!("light step expected"),
    }
    // The sorted sheet itself protected (sorting allowed): its locked cells refuse it.
    run(
        &mut e,
        &json!({ "cmd": "protectSheet", "sheet": 0, "allow": { "sort": true } }),
    );
    let before = model(&e);
    let undo_len = e.undo.len();
    let desc = json!({ "cmd": "sort", "sheet": 0, "range": rng(0, 0, 25, 4),
        "hasHeader": true, "keys": [{ "col": 2, "descending": true }] });
    assert!(e.exec_json(&desc.to_string()).is_err());
    assert_eq!(model(&e), before, "refused sort changes nothing");
    assert_eq!(e.undo.len(), undo_len);
}

/// A pivot table on sheet `Other` at C3 over the list.
fn with_pivot(g: &mut Lcg, n: u32) -> SheetEngine {
    let mut e = random_engine(g, n);
    run(
        &mut e,
        &json!({ "cmd": "pivotCreate", "sheet": 0, "range": rng(0, 0, n, 3),
            "dest": { "sheet": 1, "row": 2, "col": 2 },
            "rows": [0], "cols": [1], "values": [{ "field": 2, "function": "sum" }] }),
    );
    // A formula reading the report.
    e.set_input(1, 0, 6, "=SUM(C3:J40)").unwrap();
    e
}

// source: ECMA-376 Part 1, §18.10.1 pivotTableDefinition / pivotCacheDefinition
#[test]
fn pivot_edits_undo_redo_lightly_and_exactly() {
    for seed in 0..4u64 {
        let mut g = Lcg(seed + 700);
        let n = 8 + g.next(30) as u32;
        let mut e = with_pivot(&mut g, n);
        let cmds = [
            json!({ "cmd": "pivotUpdate", "sheet": 1, "index": 0, "rows": [0, 3], "cols": [],
                "values": [{ "field": 2, "function": "count" }],
                "filters": [{ "field": 1 }] }),
            json!({ "cmd": "pivotGroup", "sheet": 1, "index": 0, "field": 2,
                "by": "numbers", "start": -50, "end": 150, "interval": 25 }),
            json!({ "cmd": "pivotMove", "sheet": 1, "index": 0,
                "dest": { "sheet": 1, "row": 50, "col": 1 } }),
            json!({ "cmd": "pivotChangeSource", "sheet": 1, "index": 0,
                "source": format!("Data!A1:D{}", n / 2 + 1) }),
            json!({ "cmd": "pivotRefreshAll" }),
            json!({ "cmd": "pivotDelete", "sheet": 1, "index": 0 }),
        ];
        for (i, cmd) in cmds.iter().enumerate() {
            if i == 4 {
                e.set_input(0, 1, 2, "999").unwrap();
            }
            check_light(&mut e, cmd, &format!("seed {seed} {}", cmd["cmd"]));
        }
    }
}

// source: ECMA-376 Part 1, §18.10 pivot tables; §18.17.2.3 references to other sheets
#[test]
fn pivot_on_a_new_sheet_updates_formulas_naming_it() {
    let mut g = Lcg(77);
    let mut e = random_engine(&mut g, 20);
    // Reads the sheet the pivot table will create (`Sheet3`, report from A3).
    e.set_input(1, 30, 0, "=Sheet3!A3").unwrap();
    e.set_input(1, 31, 0, "=COUNTA(Sheet3!A:A)").unwrap();
    let create = json!({ "cmd": "pivotCreate", "sheet": 0, "range": rng(0, 0, 20, 3),
        "rows": [0], "values": [{ "field": 2, "function": "sum" }] });
    check_light(&mut e, &create, "new sheet");
    let after = model(&e);
    assert_eq!(after.sheets.len(), 3);
    assert_ne!(
        after.sheets[1].cells[&(31, 0)].value,
        bo_model::sheet::CellValue::Number(0.0),
        "the formula reads the new sheet"
    );
}

// source: ECMA-376 Part 1, §18.2.5 definedName (_FilterDatabase, Criteria, Extract)
#[test]
fn advanced_filters_undo_redo_lightly_and_exactly() {
    for seed in 0..4u64 {
        let mut g = Lcg(seed + 300);
        let n = 8 + g.next(30) as u32;
        let mut e = random_engine(&mut g, n);
        e.set_inputs(
            1,
            &[
                (10, 0, "Vùng".into()),
                (11, 0, REGIONS[g.next(3) as usize].into()),
            ],
        )
        .unwrap();
        // Reads the extract by name once it exists.
        e.set_input(0, 0, 9, "=ROWS(Extract)").unwrap();
        let in_place = json!({ "cmd": "advancedFilter", "sheet": 0, "list": rng(0, 0, n, 4),
            "criteriaSheet": 1, "criteria": rng(10, 0, 11, 0) });
        check_light(&mut e, &in_place, &format!("seed {seed} in place"));
        let copy = json!({ "cmd": "advancedFilter", "sheet": 0, "list": rng(0, 0, n, 3),
            "criteriaSheet": 1, "criteria": rng(10, 0, 11, 0),
            "copyTo": rng(0, 12, 0, 12), "unique": g.next(2) == 1 });
        check_light(&mut e, &copy, &format!("seed {seed} copy"));
        // Copied again (the previous extract is cleared).
        check_light(&mut e, &copy, &format!("seed {seed} copy again"));
    }
}

// source: ECMA-376 Part 1, §18.17.7 SUBTOTAL; §18.3.1.73 row outline levels
#[test]
fn subtotals_undo_redo_lightly_and_exactly() {
    for seed in 0..4u64 {
        let mut g = Lcg(seed + 400);
        let n = 8 + g.next(30) as u32;
        let mut e = random_engine(&mut g, n);
        // Grouped by region first; formulas below the list and on `Other` move.
        run(
            &mut e,
            &json!({ "cmd": "sort", "sheet": 0, "range": rng(0, 0, n, 4),
                "hasHeader": true, "keys": [{ "col": 0 }] }),
        );
        e.set_input(0, n + 3, 1, &format!("=SUM(C2:C{})", n + 1))
            .unwrap();
        let sub = json!({ "cmd": "subtotal", "sheet": 0, "range": rng(0, 0, n, 4),
            "groupBy": 0, "function": "sum", "columns": [2] });
        check_light(&mut e, &sub, &format!("seed {seed} subtotal"));
        let remove = json!({ "cmd": "removeSubtotals", "sheet": 0, "range": rng(0, 0, 0, 0) });
        check_light(&mut e, &remove, &format!("seed {seed} remove"));
    }
}
