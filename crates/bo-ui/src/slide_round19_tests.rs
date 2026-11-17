//! Round 19 tests of the BlueSlide window: Outline View edited in place, the status bar's
//! language / accessibility items, the Accessibility pane and the Language dialog.
use super::*;
use bo_engine::Engine;

struct Fixed;
impl TextMeasure for Fixed {
    fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
        (
            text.chars().count() as f32 * style.size * 0.55,
            style.size * 1.33,
        )
    }
}

fn window() -> SlideWindow {
    let mut engine = Engine::with_bundled_fonts();
    let name = "slide-text-bullets.pptx";
    let path = format!(
        "{}/../../tests/corpus/slide/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    engine.open(&std::fs::read(path).unwrap(), name).unwrap();
    let mut w = SlideWindow::new(SlideView::new(engine).unwrap(), "en-US", name);
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1481.0,
            height: 1080.0,
        },
        &Fixed,
    );
    w
}

fn press(w: &mut SlideWindow, x: f32, y: f32) {
    w.handle(
        Input::Press {
            x,
            y,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Release { x, y }, &Fixed);
}

fn key(w: &mut SlideWindow, k: Key) {
    w.handle(Input::Key(k, Mods::default()), &Fixed);
}

fn title(w: &SlideWindow, i: usize) -> String {
    let p = w.view.engine().presentation().expect("deck");
    bo_engine::slide_edit::outline::slide_outline(&p.slides[i]).title
}

#[test]
fn outline_typing_enter_and_backspace() {
    let mut w = window();
    assert!(w.run_action("view.outline"));
    w.handle(Input::Tick, &Fixed);
    let n = w.view().slide_count();
    let t0 = title(&w, 0);
    // A press at the end of the first title puts the caret there.
    let hit = w.outline9.hits[0].clone();
    assert_eq!(hit.line, (0, None));
    let x = w.size.0 - 40.0;
    press(&mut w, x, hit.y + 4.0);
    assert_eq!(w.outline_sel, Some((0, None)));
    assert_eq!(w.outline9.at, t0.chars().count());
    assert!(w.outline9.rect.is_some(), "caret drawn");
    // Typing goes into the title, one undo step each.
    w.handle(Input::Text("!".into()), &Fixed);
    assert_eq!(title(&w, 0), format!("{t0}!"));
    key(&mut w, Key::Backspace);
    assert_eq!(title(&w, 0), t0);
    // Home, then Enter at the start of the title: a new empty slide before it.
    key(&mut w, Key::Home);
    key(&mut w, Key::Enter);
    assert_eq!(w.view().slide_count(), n + 1);
    assert_eq!(title(&w, 1), t0);
    assert_eq!(w.outline_sel, Some((1, None)));
    // Backspace at the start of that title joins it to the empty slide before.
    key(&mut w, Key::Backspace);
    assert_eq!(w.view().slide_count(), n);
    assert_eq!(title(&w, 0), t0);
    // Undo through the window brings back the two-slide state.
    assert!(w.run_action("edit.undo"));
    assert_eq!(w.view().slide_count(), n + 1);
}

#[test]
fn status_bar_language_and_accessibility() {
    let mut w = window();
    w.handle(Input::Tick, &Fixed);
    // The language item names the presentation's default language when nothing is edited.
    assert_ne!(w.status9.lang, "");
    assert!(w.status9.a11y.starts_with("Accessibility: "));
    // A press on the accessibility item opens the pane, 400 px at 125 %.
    let [x, y, bw, bh] = w.status9.rects[2];
    press(&mut w, x + bw / 2.0, y + bh / 2.0);
    assert_eq!(w.side.kind, selpane::SideKind::Accessibility);
    assert!(
        (w.side.rect()[2] - 400.0).abs() < 1.0,
        "{:?}",
        w.side.rect()
    );
    // The list is longer than the pane: a scroll bar on its right edge; a press on its
    // lower arrow scrolls one row.
    let (track, _) = w.a9_bar().expect("scroll bar");
    press(&mut w, track[0] + 4.0, track[1] + track[3] - 4.0);
    assert!(w.a11y9.scroll > 0.0);
    w.a11y9.scroll = 0.0;
    // The switch keeps only the current slide's issues.
    let all = w.a11y9.issues.len();
    let r = w.side.rect();
    press(&mut w, r[0] + 40.0, r[1] + 302.0);
    assert!(w.a11y9.current_only);
    assert!(w.a9_shown().len() <= all);
    // A press on the language item opens the Language dialog; Cancel closes it.
    let [x, y, bw, bh] = w.status9.rects[1];
    press(&mut w, x + bw / 2.0, y + bh / 2.0);
    assert!(matches!(
        w.form8.as_ref().map(|o| o.kind),
        Some(forms8::Dlg::Language)
    ));
    key(&mut w, Key::Escape);
    assert!(w.form8.is_none());
}

#[test]
fn thesaurus_long_list_scrolls() {
    let mut w = window();
    w.side.thes.word = "run".into();
    assert!(w.open_side(selpane::SideKind::Thesaurus));
    w.handle(Input::Tick, &Fixed);
    // "run" has more rows than the list holds: its scroll bar's lower arrow is a hit.
    assert!(w
        .side
        .hits
        .iter()
        .any(|(_, h)| *h == selpane::SideHit::ThesScroll(1)));
    assert!(w.thesaurus_hit(&selpane::SideHit::ThesScroll(1), 1));
    w.handle(Input::Tick, &Fixed);
    assert_eq!(w.side.thes.scroll, 1);
    // The wheel over the pane scrolls three rows.
    let r = w.side.rect();
    w.handle(
        Input::Move {
            x: r[0] + 40.0,
            y: r[1] + 200.0,
        },
        &Fixed,
    );
    w.handle(Input::Wheel(1.0), &Fixed);
    assert_eq!(w.side.thes.scroll, 4);
}

#[test]
fn language_dialog_marks_the_slide() {
    let mut w = window();
    w.handle(Input::Tick, &Fixed);
    assert!(w.open_language_dialog());
    // Slide scope, "Do not check spelling".
    if let Some(open) = w.form8.as_mut() {
        open.radios[0].chosen = 1;
        open.form.set_on("noproof", true);
    }
    assert!(w.form8_event(&sform::Event::Ok));
    let p = w.view.engine().presentation().expect("deck");
    let s = &p.slides[w.view.current()];
    let marked = s.data.shapes.iter().any(|n| match n {
        bo_model::slide::ShapeNode::Shape(sh) => sh.text.as_ref().is_some_and(|t| {
            t.paragraphs.iter().flat_map(|p| &p.runs).any(|r| match r {
                bo_model::slide::TextRun::Run { props, .. } => {
                    props.attrs.iter().any(|(k, v)| k == "noProof" && v == "1")
                }
                _ => false,
            })
        }),
        _ => false,
    });
    assert!(marked);
}
