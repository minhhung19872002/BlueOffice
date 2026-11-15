//! BlueDoc window: cost of one keystroke (handle + refresh + paint calls), headless.
use super::*;
use bo_engine::Engine;
use std::cell::Cell;

/// Fixed-width measure that counts its calls.
#[derive(Default)]
struct Counting {
    calls: Cell<usize>,
}
impl TextMeasure for Counting {
    fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
        self.calls.set(self.calls.get() + 1);
        (
            text.chars().count() as f32 * style.size * 0.55,
            style.size * 1.33,
        )
    }
}

#[derive(Default)]
struct Calls {
    rects: usize,
    texts: usize,
    paths: usize,
}
impl Painter for Calls {
    fn clear(&mut self, _: Rgba) {}
    fn fill_rect(&mut self, _: f32, _: f32, _: f32, _: f32, _: Rgba) {
        self.rects += 1;
    }
    fn page(&mut self, _: usize, _: i32, _: i32) {}
    fn text(&mut self, _: &str, _: f32, _: f32, _: TextStyle, _: Rgba) {
        self.texts += 1;
    }
    fn path(&mut self, _: &[PathSeg], _: Option<Rgba>, _: Option<(Rgba, f32)>) {
        self.paths += 1;
    }
    fn push_clip(&mut self, _: f32, _: f32, _: f32, _: f32) {}
    fn pop_clip(&mut self) {}
}

/// A page store that keeps nothing (the cost of preparing pages without drawing them).
struct NoStore;
impl crate::PageStore for NoStore {
    fn update(
        &mut self,
        _: usize,
        _: &bo_render::PageList,
        _: &Engine,
        _: f32,
        _: Option<[u32; 4]>,
    ) -> Result<(), String> {
        Ok(())
    }
    fn remove(&mut self, _: usize) {}
}

fn sample(path: &str) -> DocWindow {
    let mut engine = Engine::with_bundled_fonts();
    let bytes = std::fs::read(path).unwrap();
    engine.open(&bytes, "s.docx").unwrap();
    let mut w = DocWindow::new(DocView::new(engine).unwrap(), "en-US", "s.docx");
    let m = Counting::default();
    w.handle(Input::Scale(1.25), &m);
    w.handle(
        Input::Resize {
            width: 1500.0,
            height: 1102.0,
        },
        &m,
    );
    w
}

/// A backend keeping one image per layer, keyed like the Direct2D one.
#[derive(Default)]
struct Layers {
    keys: std::collections::HashMap<usize, u64>,
    hits: Vec<usize>,
    drawn: Vec<usize>,
}
impl Painter for Layers {
    fn clear(&mut self, _: Rgba) {}
    fn fill_rect(&mut self, _: f32, _: f32, _: f32, _: f32, _: Rgba) {}
    fn page(&mut self, _: usize, _: i32, _: i32) {}
    fn text(&mut self, _: &str, _: f32, _: f32, _: TextStyle, _: Rgba) {}
    fn path(&mut self, _: &[PathSeg], _: Option<Rgba>, _: Option<(Rgba, f32)>) {}
    fn push_clip(&mut self, _: f32, _: f32, _: f32, _: f32) {}
    fn pop_clip(&mut self) {}
    fn layer_begin(&mut self, id: usize, _: [f32; 4], key: u64) -> bool {
        if self.keys.get(&id) == Some(&key) {
            self.hits.push(id);
            return true;
        }
        self.keys.insert(id, key);
        self.drawn.push(id);
        false
    }
}

/// Typing inside a word redraws neither the title bar and ribbon nor the status bar
/// (their cached layers still match), and the status bar is not measured again.
#[test]
fn typing_reuses_the_ribbon_and_status_layers() {
    let mut w = sample(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/corpus/doc/phase1-sample.docx"
    ));
    let m = Counting::default();
    let mut p = Layers::default();
    // The first keys change the title (modified) and Undo; then the chrome stays.
    for _ in 0..2 {
        w.handle(Input::Text("x".into()), &m);
        w.refresh(&m);
        w.paint(&mut p);
    }
    let measured = m.calls.get();
    p.hits.clear();
    p.drawn.clear();
    w.handle(Input::Text("y".into()), &m);
    w.refresh(&m);
    w.paint(&mut p);
    assert_eq!(p.hits, [0, 1], "drawn again: {:?}", p.drawn);
    assert_eq!(m.calls.get(), measured, "status bar measured again");
    // A ribbon change (Bold) draws the chrome layer again.
    w.action("format.bold");
    w.paint(&mut p);
    assert!(p.drawn.contains(&0));
}

/// Only the pages in view are checked as the view settles; a misspelt word far below is
/// found by the background sweep, a few milliseconds per tick, and shows in the status
/// bar's proofing state.
#[test]
fn background_sweep_finds_issues_far_from_the_view() {
    let mut text: Vec<String> = (0..400)
        .map(|i| format!("Đoạn văn số {i} có nội dung đúng chính tả và khá dài để lấp trang."))
        .collect();
    text.push("Lỗi ở cuối: documnet.".into());
    let mut engine = Engine::with_bundled_fonts();
    engine
        .load(
            bo_engine::plain_text_document(&text.join("\n")),
            "long.docx",
        )
        .unwrap();
    let mut w = DocWindow::new(DocView::new(engine).unwrap(), "en-US", "long.docx");
    let m = Counting::default();
    w.handle(Input::Scale(1.0), &m);
    w.handle(
        Input::Resize {
            width: 1200.0,
            height: 800.0,
        },
        &m,
    );
    assert!(w.view.page_count() > 8, "{} pages", w.view.page_count());
    w.view.refresh_spelling();
    assert_eq!(w.view.proofing_issues(), 0, "nothing wrong in view");
    assert!(!w.view.document_has_issues());
    let mut ticks = 0;
    while w.view.sweeping() {
        w.handle(Input::Tick, &m);
        ticks += 1;
        assert!(ticks < 1000);
    }
    assert!(
        w.view.document_has_issues(),
        "the last page's mistake is known"
    );
    assert_eq!(
        w.view.proofing_issues(),
        0,
        "the view still shows its own pages"
    );
}

/// A misspelt word typed on page 250 of `BO_PERF_DOC` (a 300-page document) is found by
/// the background sweep while the view is back on page 1; prints the sweep's cost.
#[test]
#[ignore = "needs BO_PERF_DOC, run by hand"]
fn sweep_finds_a_mistake_on_page_250() {
    let Ok(path) = std::env::var("BO_PERF_DOC") else {
        return;
    };
    let mut w = sample(&path);
    let m = Counting::default();
    w.view.show_page(249);
    let (x, y) = (700.0, 500.0);
    w.handle(
        Input::Press {
            x,
            y,
            count: 1,
            shift: false,
        },
        &m,
    );
    w.handle(Input::Release { x, y }, &m);
    let page = w.view.current_page();
    w.handle(Input::Text(" documnet ".into()), &m);
    w.view.show_page(0);
    w.view.refresh_spelling();
    println!(
        "typed on page {}; issues in view: {}",
        page + 1,
        w.view.proofing_issues()
    );
    let (mut steps, mut longest) = (0, 0.0f64);
    let t = std::time::Instant::now();
    while w.view.sweeping() {
        let s = std::time::Instant::now();
        w.view.spell_sweep(std::time::Duration::from_millis(3));
        longest = longest.max(s.elapsed().as_secs_f64() * 1e3);
        steps += 1;
    }
    println!(
        "sweep: {steps} steps, {:.0} ms in all, longest {longest:.2} ms; document has issues: {}",
        t.elapsed().as_secs_f64() * 1e3,
        w.view.document_has_issues()
    );
    assert!(w.view.document_has_issues());
    // Typing again restarts the sweep; the next one is cheaper (cached paragraphs).
    w.handle(Input::Text("x".into()), &m);
    let t = std::time::Instant::now();
    while w.view.sweeping() {
        w.view.spell_sweep(std::time::Duration::from_millis(3));
    }
    println!("second sweep: {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
}

/// Time typing `n` characters as the host does (handle, refresh, paint); prints a
/// breakdown. `cargo test -p bo-ui --release keystroke_cost -- --ignored --nocapture`
/// (`BO_PERF_DOC` picks another document).
#[test]
#[ignore = "timing, run by hand"]
fn keystroke_cost() {
    let path = std::env::var("BO_PERF_DOC").unwrap_or_else(|_| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/corpus/doc/phase1-sample.docx"
        )
        .to_owned()
    });
    let mut w = sample(&path);
    let m = Counting::default();
    // Caret where the typing script clicks (client 600, 612).
    w.handle(
        Input::Press {
            x: 600.0,
            y: 612.0,
            count: 1,
            shift: false,
        },
        &m,
    );
    w.handle(Input::Release { x: 600.0, y: 612.0 }, &m);
    // Spelling marks present, as after the idle check.
    let t = std::time::Instant::now();
    w.view.refresh_spelling();
    println!(
        "first spelling check: {:.2} ms, {} marks",
        t.elapsed().as_secs_f64() * 1e3,
        w.view.proofing_issues()
    );
    let n: u32 = 200;
    let (mut handle, mut refresh, mut paint, mut prepare) = (0.0, 0.0, 0.0, 0.0);
    let mut calls = Calls::default();
    for i in 0..n {
        let c = if i % 7 == 6 { " " } else { "a" };
        let t = std::time::Instant::now();
        w.handle(Input::Text(c.into()), &m);
        let t1 = std::time::Instant::now();
        w.refresh(&m);
        let t2 = std::time::Instant::now();
        w.prepare(&mut NoStore).unwrap();
        let t2b = std::time::Instant::now();
        let mut p = Calls::default();
        w.paint(&mut p);
        let t3 = std::time::Instant::now();
        handle += (t1 - t).as_secs_f64();
        refresh += (t2 - t1).as_secs_f64();
        prepare += (t2b - t2).as_secs_f64();
        paint += (t3 - t2b).as_secs_f64();
        calls = p;
    }
    println!(
        "prepare (pages without drawing): {:.3} ms",
        prepare * 1e3 / f64::from(n)
    );
    // The idle spelling check after typing (only the edited paragraph is new).
    let t = std::time::Instant::now();
    w.view.refresh_spelling();
    let idle = t.elapsed().as_secs_f64() * 1e3;
    let t = std::time::Instant::now();
    w.view.refresh_spelling();
    let again = t.elapsed().as_secs_f64() * 1e3;
    println!("idle spelling check: {idle:.2} ms after typing, {again:.2} ms unchanged");
    // Background sweep of the whole document: longest step (what a key can wait for).
    let (mut steps, mut longest) = (0, 0.0f64);
    let t = std::time::Instant::now();
    while w.view.sweeping() {
        let s = std::time::Instant::now();
        w.view.spell_sweep(std::time::Duration::from_millis(3));
        longest = longest.max(s.elapsed().as_secs_f64() * 1e3);
        steps += 1;
    }
    println!(
        "sweep: {steps} steps, {:.0} ms in all, longest {longest:.2} ms, issues {}",
        t.elapsed().as_secs_f64() * 1e3,
        w.view.document_has_issues()
    );
    let ms = |v: f64| v * 1e3 / f64::from(n);
    println!(
        "per key: handle {:.3} ms, refresh {:.3} ms, paint {:.3} ms; measures {}, rects {}, texts {}, paths {}",
        ms(handle),
        ms(refresh),
        ms(paint),
        m.calls.get() / n as usize,
        calls.rects,
        calls.texts,
        calls.paths
    );
}
