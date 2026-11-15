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
    // Spelling marks present, as after the idle check.
    let t = std::time::Instant::now();
    w.view.refresh_spelling();
    println!(
        "first spelling check: {:.2} ms, {} marks",
        t.elapsed().as_secs_f64() * 1e3,
        w.view.proofing_issues()
    );
    let n = 200;
    let (mut handle, mut refresh, mut paint) = (0.0, 0.0, 0.0);
    let mut calls = Calls::default();
    for i in 0..n {
        let c = if i % 7 == 6 { " " } else { "a" };
        let t = std::time::Instant::now();
        w.handle(Input::Text(c.into()), &m);
        let t1 = std::time::Instant::now();
        w.refresh(&m);
        let t2 = std::time::Instant::now();
        let mut p = Calls::default();
        w.paint(&mut p);
        let t3 = std::time::Instant::now();
        handle += (t1 - t).as_secs_f64();
        refresh += (t2 - t1).as_secs_f64();
        paint += (t3 - t2).as_secs_f64();
        calls = p;
    }
    // The idle spelling check after typing (only the edited paragraph is new).
    let t = std::time::Instant::now();
    w.view.refresh_spelling();
    let idle = t.elapsed().as_secs_f64() * 1e3;
    let t = std::time::Instant::now();
    w.view.refresh_spelling();
    let again = t.elapsed().as_secs_f64() * 1e3;
    println!("idle spelling check: {idle:.2} ms after typing, {again:.2} ms unchanged");
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
