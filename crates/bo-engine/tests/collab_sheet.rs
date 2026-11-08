//! BlueSheet live collaboration (Phase 6): two sheet engines with their own
//! `bo-collab` replicas exchanging real y-sync frames; concurrent edits —
//! cell inputs, formats, row/column inserts and deletes with formula
//! references, merges, sizes, sheets, rules — converge.

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
