//! Cost of one typed character in a long document, split by step (run by hand:
//! `BO_PERF_DOC=big.docx cargo test --release -p bo-engine --lib typing_cost -- --ignored --nocapture`).
use crate::edit::Command;
use crate::Engine;
use std::time::Instant;

fn time(label: &str, n: u32, mut f: impl FnMut()) {
    let t = Instant::now();
    for _ in 0..n {
        f();
    }
    println!(
        "{label:<28} {:8.3} ms",
        t.elapsed().as_secs_f64() * 1e3 / f64::from(n)
    );
}

#[test]
#[ignore = "timing, run by hand"]
fn typing_cost() {
    let Ok(path) = std::env::var("BO_PERF_DOC") else {
        return;
    };
    let mut e = Engine::with_bundled_fonts();
    e.open(&std::fs::read(path).unwrap(), "big.docx").unwrap();
    // Caret in the middle of the document.
    let paras = e
        .session
        .as_ref()
        .map_or(0, |s| bo_model::index::body_paragraphs(s.doc()).len());
    let sel = bo_edit::Selection::caret(bo_edit::Pos {
        para: paras / 2,
        offset: 0,
    });
    e.command(Command::SetSelection { selection: sel }).unwrap();
    let n = 50;
    time("insert text (whole command)", n, || {
        e.command(Command::InsertText { text: "a".into() }).unwrap();
    });
    let s = e.session.as_ref().unwrap();
    time("revisions", n, || drop(s.revisions()));
    time("comments", n, || drop(s.comments()));
    time("format state", n, || drop(s.format_state()));
    time("protection info", n, || drop(crate::forms::info(s)));
    time("selected equation", n, || drop(s.selected_equation()));
    time("selected ruby", n, || drop(s.selected_ruby()));
    time("control at caret", n, || drop(s.control_at_caret()));
    time("block control at caret", n, || {
        drop(s.block_control_at_caret())
    });
    time("field at", n, || drop(s.field_at()));
    time("form field info", n, || drop(s.form_field_info()));
    time("picked format", n, || drop(s.picked_format().is_some()));
    let layout = e.layout.document();
    let sel = s.selection();
    time("caret rect", n, || {
        let _ = crate::nav::caret_rect(layout, sel.focus);
    });
    time("selection rects", n, || {
        drop(crate::nav::selection_rects(layout, sel));
    });
    time("review state", n, || {
        drop(crate::review::review_state(s, &e.markup));
    });
    time("info", n, || drop(e.info()));
    time("is modified", n, || {
        let _ = e.is_modified();
    });
    time("edit result", n, || {
        drop(e.edit_result(Vec::new(), (None, None, None), None));
    });
    let mut dirty = Vec::new();
    time("refresh markup", n, || e.refresh_markup(&mut dirty));
    time("relayout (nothing changed)", n, || drop(e.relayout()));
    time("spelling marks, 3 pages", 5, || {
        drop(e.spelling_marks(Some(&[10, 11, 12])));
    });
}
