//! BlueSheet live collaboration (Phase 6): two sheet engines with their own
//! `bo-collab` replicas exchanging real y-sync frames; concurrent edits —
//! cell inputs, formats, row/column inserts and deletes with formula
//! references, merges, sizes, sheets, rules — converge.
// Tests may unwrap: a panic is a test failure.
#![allow(clippy::unwrap_used)]

use bo_collab::{Change, Session};
use bo_engine::{CollabChange, SheetEngine};
use proptest::prelude::*;

struct Peer {
    engine: SheetEngine,
    replica: Session,
}

fn to_replica(c: &CollabChange) -> Change {
    serde_json::from_value(serde_json::to_value(c).unwrap()).unwrap()
}

fn to_engine(c: &Change) -> CollabChange {
    serde_json::from_value(serde_json::to_value(c).unwrap()).unwrap()
}

impl Peer {
    fn new(id: u64, cells: &[(u32, u32, &str)]) -> Peer {
        let mut engine = SheetEngine::new();
        engine.set_locale("en-US");
        engine.new_workbook("chung.xlsx").unwrap();
        let v: Vec<(u32, u32, String)> = cells
            .iter()
            .map(|(r, c, s)| (*r, *c, (*s).to_owned()))
            .collect();
        engine.set_inputs(0, &v).unwrap();
        Peer {
            engine,
            replica: Session::with_client_id(id),
        }
    }

    fn share(&mut self) {
        let c = self.engine.collab_local().unwrap();
        self.replica.apply_local(&to_replica(&c)).unwrap();
    }

    fn input(&mut self, sheet: usize, r: u32, c: u32, s: &str) {
        self.engine.set_input(sheet, r, c, s).unwrap();
        self.share();
    }

    fn exec(&mut self, json: &str) {
        self.engine.exec_json(json).unwrap();
        self.share();
    }

    fn structural(&mut self, rows: bool, at: u32, n: u32, delete: bool) {
        self.engine.structural(0, rows, at, n, delete).unwrap();
        self.share();
    }

    fn pull(&mut self) {
        let ch = self.replica.take_remote();
        if ch.is_empty() {
            return;
        }
        self.engine.collab_apply(&to_engine(&ch)).unwrap();
        // Applying never produces new local changes.
        let back = self.engine.collab_local().unwrap();
        assert!(back.is_empty(), "echo: {back:?}");
    }

    fn input_at(&self, sheet: usize, r: u32, c: u32) -> String {
        self.engine.cell(sheet, r, c).unwrap().input
    }

    /// Every sheet: name, cells (input + style json), merges, rows, cols.
    fn view(&self) -> String {
        let wb = self.engine.workbook().unwrap();
        let mut out = String::new();
        for (i, ws) in wb.sheets.iter().enumerate() {
            out.push_str(&format!("# {} {:?}\n", ws.name, ws.merges));
            for (&(r, c), cell) in &ws.cells {
                let st = wb.styles.cell_xfs.get(cell.style as usize).cloned();
                let font = st
                    .as_ref()
                    .and_then(|x| wb.styles.fonts.get(x.font_id as usize).cloned());
                out.push_str(&format!(
                    "{r},{c} {:?} {:?} {:?}\n",
                    self.input_at(i, r, c),
                    font.map(|f| f.bold),
                    self.engine.cell(i, r, c).unwrap().display
                ));
            }
            out.push_str(&format!("rows {:?}\ncols {:?}\n", ws.rows, ws.cols));
        }
        out
    }
}

fn send(from: &mut Peer, to: &mut Peer) {
    for f in from.replica.drain_outbox() {
        for r in to.replica.receive(&f).unwrap() {
            from.replica.receive(&r).unwrap();
        }
    }
}

fn settle(a: &mut Peer, b: &mut Peer) {
    for _ in 0..4 {
        send(a, b);
        send(b, a);
        a.pull();
        b.pull();
    }
}

fn pair(cells: &[(u32, u32, &str)]) -> (Peer, Peer) {
    let mut a = Peer::new(1, cells);
    let mut b = Peer::new(2, &[(9, 9, "khác")]);
    let c = a.engine.collab_start(true, 1).unwrap();
    a.replica.seed(&to_replica(&c)).unwrap();
    b.engine.collab_start(false, 2).unwrap();
    let s = a.replica.start().unwrap();
    for r in b.replica.receive(&s).unwrap() {
        a.replica.receive(&r).unwrap();
    }
    let s = b.replica.start().unwrap();
    for r in a.replica.receive(&s).unwrap() {
        b.replica.receive(&r).unwrap();
    }
    settle(&mut a, &mut b);
    // Late joiner: the shared state replaces its copy.
    let full = b.replica.full_state();
    b.engine.collab_apply(&to_engine(&full)).unwrap();
    (a, b)
}

#[test]
fn late_joiner_gets_the_workbook() {
    let (a, b) = pair(&[(0, 0, "Tổng"), (0, 1, "=2*21"), (2, 3, "1.5")]);
    assert_eq!(b.input_at(0, 0, 1), "=2*21");
    assert_eq!(b.engine.cell(0, 0, 1).unwrap().display, "42");
    assert_eq!(b.input_at(0, 9, 9), "");
    assert_eq!(a.view(), b.view());
}

#[test]
fn row_insert_and_concurrent_formula_converge() {
    let (mut a, mut b) = pair(&[(0, 0, "1"), (1, 0, "2"), (2, 0, "3"), (4, 0, "=SUM(A1:A3)")]);
    // A inserts a row above row 2 while B writes a formula pointing at A3
    // and a value in row 4.
    a.structural(true, 1, 1, false);
    b.input(0, 0, 1, "=A3*10");
    b.input(0, 3, 0, "x");
    settle(&mut a, &mut b);
    assert_eq!(a.view(), b.view());
    // B's reference followed the row it meant (A3 → A4).
    assert_eq!(a.input_at(0, 0, 1), "=A4*10");
    assert_eq!(a.engine.cell(0, 0, 1).unwrap().display, "30");
    // B's value landed in the row it was typed in (shifted down by A's insert).
    assert_eq!(a.input_at(0, 4, 0), "x");
    assert_eq!(a.input_at(0, 5, 0), "=SUM(A1:A4)");
    // Concurrent delete of a referenced row and a column insert.
    a.structural(true, 3, 1, true);
    b.structural(false, 0, 1, false);
    settle(&mut a, &mut b);
    assert_eq!(a.view(), b.view());
    assert_eq!(a.input_at(0, 0, 2), "=#REF!*10");
}

#[test]
fn formats_sizes_merges_sheets_and_rules_converge() {
    let (mut a, mut b) = pair(&[(0, 0, "a"), (0, 1, "b")]);
    // Value and format of the same cell at once: both survive.
    a.exec(
        r#"{"cmd":"format","sheet":0,"range":{"r0":0,"c0":0,"r1":0,"c1":0},"patch":{"bold":true}}"#,
    );
    b.input(0, 0, 0, "mới");
    settle(&mut a, &mut b);
    assert_eq!(a.view(), b.view());
    assert!(a.view().contains("\"mới\" Some(true)"), "{}", a.view());
    // Sizes and merges.
    a.exec(r#"{"cmd":"resize","sheet":0,"rows":false,"first":1,"last":1,"size":120}"#);
    b.exec(r#"{"cmd":"merge","sheet":0,"range":{"r0":2,"c0":0,"r1":3,"c1":1}}"#);
    b.exec(r#"{"cmd":"resize","sheet":0,"rows":true,"first":5,"last":5,"size":40}"#);
    settle(&mut a, &mut b);
    assert_eq!(a.view(), b.view());
    assert_eq!(a.engine.workbook().unwrap().sheets[0].merges.len(), 1);
    // Sheets: both add one, A renames the first, then B moves nothing.
    a.engine.add_sheet(Some("Thu")).unwrap();
    a.share();
    b.engine.add_sheet(Some("Chi")).unwrap();
    b.share();
    settle(&mut a, &mut b);
    b.engine.rename_sheet(0, "Tổng hợp").unwrap();
    b.share();
    a.input(1, 0, 0, "trên sheet Thu");
    settle(&mut a, &mut b);
    assert_eq!(a.view(), b.view());
    let names = |p: &Peer| {
        p.engine
            .workbook()
            .unwrap()
            .sheets
            .iter()
            .map(|s| s.name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&a).len(), 3);
    assert_eq!(names(&a)[0], "Tổng hợp");
    a.engine.delete_sheet(2).unwrap();
    a.share();
    settle(&mut a, &mut b);
    assert_eq!(names(&a), names(&b));
    assert_eq!(names(&b).len(), 2);
    // Conditional formats and data validation (last writer wins per sheet).
    a.exec(r#"{"cmd":"addCf","sheet":0,"range":{"r0":0,"c0":0,"r1":9,"c1":0},"rule":{"kind":"cellIs","operator":"greaterThan","formula1":"5","style":{"fill":"FFC7CE"}}}"#);
    settle(&mut a, &mut b);
    let cf = |p: &Peer| {
        p.engine.workbook().unwrap().sheets[0]
            .conditional_formats
            .len()
    };
    assert_eq!(cf(&a), cf(&b));
}

#[test]
fn undo_reverts_only_own_changes() {
    let (mut a, mut b) = pair(&[(0, 0, "1")]);
    a.input(0, 1, 0, "của An");
    settle(&mut a, &mut b);
    b.replica.stop_capturing();
    b.input(0, 2, 0, "của Bình");
    b.structural(true, 0, 1, false);
    settle(&mut a, &mut b);
    // B undoes the row insert, then its cell: A's cell stays.
    assert!(b.replica.undo());
    b.pull();
    settle(&mut a, &mut b);
    assert_eq!(a.view(), b.view());
    assert_eq!(a.input_at(0, 1, 0), "của An");
}

// ------------------------------------------------------------- property

#[derive(Debug, Clone)]
enum Edit {
    Input(u32, u32, u8),
    Formula(u32, u32, u32, u32),
    Rows(u32, bool),
    Cols(u32, bool),
    Bold(u32, u32),
}

fn edit() -> impl Strategy<Value = Edit> {
    let n = 0u32..6;
    prop_oneof![
        (n.clone(), n.clone(), 0u8..9).prop_map(|(r, c, v)| Edit::Input(r, c, v)),
        (n.clone(), n.clone(), n.clone(), n.clone())
            .prop_map(|(r, c, x, y)| Edit::Formula(r, c, x, y)),
        (n.clone(), any::<bool>()).prop_map(|(r, d)| Edit::Rows(r, d)),
        (n.clone(), any::<bool>()).prop_map(|(c, d)| Edit::Cols(c, d)),
        (n.clone(), n.clone()).prop_map(|(r, c)| Edit::Bold(r, c)),
    ]
}

fn col(c: u32) -> char {
    char::from(b'A' + u8::try_from(c).unwrap())
}

fn apply(p: &mut Peer, e: &Edit) {
    match e {
        Edit::Input(r, c, v) => p.input(0, *r, *c, &v.to_string()),
        Edit::Formula(r, c, x, y) => {
            p.input(0, *r, *c, &format!("={}{}+1", col(*y), x + 1));
        }
        Edit::Rows(r, d) => p.structural(true, *r, 1, *d),
        Edit::Cols(c, d) => p.structural(false, *c, 1, *d),
        Edit::Bold(r, c) => p.exec(&format!(
            r#"{{"cmd":"format","sheet":0,"range":{{"r0":{r},"c0":{c},"r1":{r},"c1":{c}}},"patch":{{"bold":true}}}}"#
        )),
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]

    #[test]
    fn random_concurrent_sheet_edits_converge(
        rounds in prop::collection::vec(
            (prop::collection::vec(edit(), 0..4), prop::collection::vec(edit(), 0..4)),
            1..4,
        )
    ) {
        let (mut a, mut b) = pair(&[(0, 0, "1"), (1, 1, "=A1*2"), (2, 2, "x")]);
        for (ea, eb) in &rounds {
            for e in ea { apply(&mut a, e); }
            for e in eb { apply(&mut b, e); }
            settle(&mut a, &mut b);
            prop_assert_eq!(a.view(), b.view());
        }
    }
}

// source: ECMA-376 Part 1, §18.3.1.18 conditionalFormatting / §18.3.1.32 dataValidation sqref
#[test]
fn rules_added_during_a_row_insert_follow_the_rows() {
    let (mut a, mut b) = pair(&[(0, 0, "1")]);
    settle(&mut a, &mut b);
    // A adds rules on rows 5..10 while B inserts two rows at the top.
    a.exec(r#"{"cmd":"addCf","sheet":0,"range":{"r0":4,"c0":0,"r1":9,"c1":0},"rule":{"kind":"cellIs","operator":"greaterThan","formula1":"5","style":{"fill":"FFC7CE"}}}"#);
    a.exec(r#"{"cmd":"setValidation","sheet":0,"range":{"r0":4,"c0":2,"r1":9,"c1":2},"spec":{"kind":"whole","operator":"greaterThan","formula1":"0"}}"#);
    b.structural(true, 0, 2, false);
    settle(&mut a, &mut b);
    let ranges = |p: &Peer| {
        let ws = &p.engine.workbook().unwrap().sheets[0];
        (
            ws.conditional_formats
                .iter()
                .flat_map(|c| c.sqref.iter().map(ToString::to_string))
                .collect::<Vec<_>>(),
            ws.data_validations
                .iter()
                .flat_map(|d| d.sqref.iter().map(ToString::to_string))
                .collect::<Vec<_>>(),
        )
    };
    assert_eq!(ranges(&a), ranges(&b));
    assert_eq!(
        ranges(&a),
        (vec!["A7:A12".to_owned()], vec!["C7:C12".to_owned()])
    );
}

// source: ECMA-376 Part 1, §18.3.1.2 autoFilter ref
#[test]
fn a_filter_turned_on_during_a_row_insert_follows_the_rows() {
    let cells = [
        (4, 0, "Tên"),
        (4, 1, "Số"),
        (5, 0, "a"),
        (5, 1, "1"),
        (6, 0, "b"),
        (6, 1, "2"),
        (7, 0, "c"),
        (7, 1, "3"),
    ];
    let (mut a, mut b) = pair(&cells);
    settle(&mut a, &mut b);
    a.exec(r#"{"cmd":"toggleFilter","sheet":0,"range":{"r0":5,"c0":0,"r1":5,"c1":0}}"#);
    b.structural(true, 0, 2, false);
    settle(&mut a, &mut b);
    let filter = |p: &Peer| {
        p.engine.workbook().unwrap().sheets[0]
            .auto_filter
            .as_ref()
            .map(|f| f.range.to_string())
    };
    assert_eq!(filter(&a), filter(&b));
    assert_eq!(filter(&a).as_deref(), Some("A7:B10"));
    assert_eq!(a.view(), b.view());
}

// source: ECMA-376 Part 1, §20.5.2.33 twoCellAnchor, §21.2.2.215 f (series references)
#[test]
fn a_chart_added_during_a_row_insert_follows_the_rows() {
    let cells = [
        (4, 0, "Tháng"),
        (4, 1, "Doanh thu"),
        (5, 0, "1"),
        (5, 1, "10"),
        (6, 0, "2"),
        (6, 1, "20"),
    ];
    let (mut a, mut b) = pair(&cells);
    settle(&mut a, &mut b);
    a.exec(
        r#"{"cmd":"insertChart","sheet":0,"range":{"r0":4,"c0":0,"r1":6,"c1":1},"kind":"column"}"#,
    );
    b.structural(true, 0, 2, false);
    settle(&mut a, &mut b);
    let chart = |p: &Peer| {
        let ch = p.engine.workbook().unwrap().sheets[0].charts[0].clone();
        let refs: Vec<Option<String>> = ch
            .plots
            .iter()
            .flat_map(|pl| &pl.series)
            .flat_map(|s| [s.cat_ref.clone(), s.val_ref.clone()])
            .collect();
        (ch.from, ch.to, refs)
    };
    assert_eq!(chart(&a), chart(&b), "{:?} / {:?}", chart(&a), chart(&b));
    // The data moved two rows down on both replicas: so did the references.
    let (_, _, refs) = chart(&a);
    assert!(
        refs.iter()
            .flatten()
            .all(|r| r.contains("$7") || r.contains("$8") || r.contains("$9")),
        "{refs:?}"
    );
}

#[test]
fn a_filter_and_a_row_insert_from_the_same_peer_shift_once() {
    let cells = [(4, 0, "Tên"), (4, 1, "Số"), (5, 0, "a"), (5, 1, "1")];
    let (mut a, mut b) = pair(&cells);
    settle(&mut a, &mut b);
    // A turns the filter on, then inserts two rows at the top; B gets both at once.
    a.exec(r#"{"cmd":"toggleFilter","sheet":0,"range":{"r0":5,"c0":0,"r1":5,"c1":0}}"#);
    a.structural(true, 0, 2, false);
    settle(&mut a, &mut b);
    let filter = |p: &Peer| {
        p.engine.workbook().unwrap().sheets[0]
            .auto_filter
            .as_ref()
            .map(|f| f.range.to_string())
    };
    assert_eq!(filter(&a).as_deref(), Some("A7:B8"));
    assert_eq!(filter(&b), filter(&a));
}

// source: ECMA-376 Part 1, §18.2.6 definedName _xlnm.Print_Area
#[test]
fn a_print_area_set_during_a_row_insert_follows_the_rows() {
    let (mut a, mut b) = pair(&[(0, 0, "1")]);
    settle(&mut a, &mut b);
    a.exec(r#"{"cmd":"setPageSetup","sheet":0,"printArea":{"r0":4,"c0":0,"r1":9,"c1":3}}"#);
    b.structural(true, 0, 2, false);
    settle(&mut a, &mut b);
    let pa = |p: &Peer| p.engine.workbook().unwrap().sheets[0].print_area.clone();
    assert_eq!(pa(&a), pa(&b));
    assert_eq!(pa(&a).as_deref(), Some("Sheet1!$A$7:$D$12"));
}

// source: ECMA-376 Part 1, §18.10.1.49 location, §18.10.1.98 worksheetSource
#[test]
fn a_pivot_created_during_a_row_insert_follows_the_rows() {
    let cells = [
        (0, 0, "Vùng"),
        (0, 1, "Doanh thu"),
        (1, 0, "Bắc"),
        (1, 1, "10"),
        (2, 0, "Nam"),
        (2, 1, "5"),
        (3, 0, "Bắc"),
        (3, 1, "7"),
    ];
    let (mut a, mut b) = pair(&cells);
    settle(&mut a, &mut b);
    a.exec(r#"{"cmd":"pivotCreate","sheet":0,"range":{"r0":0,"c0":0,"r1":3,"c1":1},"dest":{"sheet":0,"row":0,"col":6},"rows":[0],"values":[{"field":1,"function":"sum"}]}"#);
    b.structural(true, 0, 2, false);
    settle(&mut a, &mut b);
    let pivot = |p: &Peer| {
        let wb = p.engine.workbook().unwrap();
        let pt = wb.sheets[0].pivots[0].clone();
        let src = wb
            .pivot_caches
            .iter()
            .find(|c| c.cache_id == pt.cache_id)
            .and_then(|c| c.source_ref.clone());
        (pt.location.to_string(), src)
    };
    assert_eq!(pivot(&a), pivot(&b), "{:?} / {:?}", pivot(&a), pivot(&b));
    let (loc, src) = pivot(&a);
    assert!(loc.starts_with("G3"), "{loc}");
    assert_eq!(src.as_deref(), Some("A3:B6"));
}

// source: ECMA-376 Part 1, §18.3.1.66 pane
#[test]
fn panes_frozen_during_a_row_insert_follow_the_rows() {
    let (mut a, mut b) = pair(&[(0, 0, "1")]);
    settle(&mut a, &mut b);
    a.exec(r#"{"cmd":"freeze","sheet":0,"rows":2,"cols":0}"#);
    b.structural(true, 0, 1, false);
    settle(&mut a, &mut b);
    let pane = |p: &Peer| {
        let v = &p.engine.workbook().unwrap().sheets[0].views;
        v.first()
            .and_then(|v| v.pane.clone())
            .map(|p| (p.y_split, p.x_split, p.top_left_cell))
    };
    assert_eq!(pane(&a), pane(&b));
    assert_eq!(pane(&a), Some((3.0, 0.0, Some("A4".into()))));
}

// source: [MS-XLSX] controlPr/anchor, fmlaLink; x14:sparkline xm:sqref / xm:f
#[test]
fn controls_and_sparklines_shift_once_with_a_row_insert_from_the_same_peer() {
    let (mut a, mut b) = pair(&[(0, 0, "1"), (0, 1, "2")]);
    settle(&mut a, &mut b);
    a.exec(r#"{"cmd":"insertFormControl","sheet":0,"kind":"checkBox","row":2,"col":1}"#);
    a.exec(r#"{"cmd":"formatFormControl","sheet":0,"index":0,"spec":{"link":"$D$5"}}"#);
    a.exec(r#"{"cmd":"sparklineInsert","sheet":0,"type":"line","data":"A1:B1","location":{"r0":0,"c0":2,"r1":0,"c1":2}}"#);
    settle(&mut a, &mut b);
    // A inserts two rows at the top; B receives the shifted objects and the rows at once.
    a.structural(true, 0, 2, false);
    settle(&mut a, &mut b);
    let placed = |p: &Peer| {
        let ws = &p.engine.workbook().unwrap().sheets[0];
        let c = &ws.form_controls[0];
        let s = &ws.sparkline_groups[0].sparklines[0];
        (
            c.anchor.from.to_string(),
            c.link.clone(),
            s.location.to_string(),
            s.formula.clone(),
        )
    };
    assert_eq!(
        placed(&a),
        (
            "B5".to_owned(),
            Some("$D$7".to_owned()),
            "C3".to_owned(),
            Some("Sheet1!A3:B3".to_owned())
        )
    );
    assert_eq!(placed(&b), placed(&a));
    // B inserts a column: A follows.
    b.structural(false, 0, 1, false);
    settle(&mut a, &mut b);
    assert_eq!(placed(&a).0, "C5");
    assert_eq!(placed(&a), placed(&b));
}

#[test]
fn inspector_removals_and_quick_fixes_are_shared() {
    let (mut a, mut b) = pair(&[(0, 0, "giữ"), (1, 0, "ẩn"), (2, 0, "sau")]);
    // A hides row 2 and merges A4:B4; B sees both.
    a.exec(r#"{"cmd":"hide","sheet":0,"rows":true,"first":1,"last":1,"hidden":true}"#);
    a.exec(r#"{"cmd":"merge","sheet":0,"range":{"r0":3,"c0":0,"r1":3,"c1":1}}"#);
    settle(&mut a, &mut b);
    assert_eq!(a.view(), b.view());
    // The inspector removes the hidden row (the rows below move up) …
    a.exec(r#"{"cmd":"inspectRemove","categories":["hiddenRows"]}"#);
    // … and a quick fix of the accessibility checker splits the merged cells.
    let v: serde_json::Value = serde_json::from_str(
        &a.engine
            .exec_json(
                r#"{"cmd":"inspectFix","fix":{"kind":"unmerge","target":{"kind":"range","sheet":0,"r0":2,"c0":0,"r1":2,"c1":1}}}"#,
            )
            .unwrap(),
    )
    .unwrap();
    a.share();
    assert!(v.get("error").is_none(), "{v}");
    settle(&mut a, &mut b);
    assert_eq!(b.input_at(0, 1, 0), "sau");
    assert_eq!(b.engine.workbook().unwrap().sheets[0].merges.len(), 0);
    assert_eq!(a.view(), b.view());
}
