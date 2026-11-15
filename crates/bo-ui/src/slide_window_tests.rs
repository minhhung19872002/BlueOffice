//! Tests of the BlueSlide window (headless, fixed-width text measure).
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

fn click(w: &mut SlideWindow, x: f32, y: f32) {
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

fn click_command(w: &mut SlideWindow, id: &'static str) {
    let [x, y, cw, ch] = w.command_rect(id).expect(id);
    click(w, x + cw / 2.0, y + ch / 2.0);
}

#[test]
fn ribbon_new_slide_layout_menu_and_delete() {
    let mut w = window();
    let n = w.view().slide_count();
    click_command(&mut w, "slide.new");
    assert_eq!(w.view().slide_count(), n + 1);
    click_command(&mut w, "slide.layout");
    assert!(w.menu.is_some(), "layout menu");
    w.handle(Input::Key(Key::Down, Mods::default()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(
        w.view().slide_count(),
        n + 2,
        "slide with the chosen layout"
    );
    click_command(&mut w, "slide.delete");
    assert_eq!(w.view().slide_count(), n + 1);
    // Undo from the title bar's quick access button runs through the window.
    assert!(w.run_action("edit.undo"));
    assert_eq!(w.view().slide_count(), n + 2);
}

#[test]
fn status_bar_toggles_notes_and_fits() {
    let mut w = window();
    assert!(w.view().notes_shown());
    let [x, y, bw, bh] = w.status[0];
    click(&mut w, x + bw / 2.0, y + bh / 2.0);
    assert!(!w.view().notes_shown());
    let [x, y, bw, bh] = w.status[3];
    click(&mut w, x + bw / 2.0, y + bh / 2.0);
    assert!(!w.view().fits());
    let [x, y, bw, bh] = w.status[5];
    click(&mut w, x + bw / 2.0, y + bh / 2.0);
    assert!(w.view().fits());
}

#[test]
fn f5_asks_for_full_screen_and_escape_returns() {
    let mut w = window();
    w.take_requests();
    w.handle(Input::Key(Key::F(5), Mods::default()), &Fixed);
    assert!(w.showing());
    assert_eq!(w.take_requests(), vec![Request::FullScreen(true)]);
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(!w.showing());
    assert_eq!(w.take_requests(), vec![Request::FullScreen(false)]);
    w.handle(
        Input::Key(
            Key::Letter('s'),
            Mods {
                ctrl: true,
                ..Mods::default()
            },
        ),
        &Fixed,
    );
    assert_eq!(w.take_requests(), vec![Request::Save]);
}

/// Open ribbon tab `i` (0 Home, 1 Insert, 2 Design, 3 Transitions, 4 Animations,
/// 5 Slide Show, 6 Review, 7 View).
fn tab(w: &mut SlideWindow, i: usize) {
    let [x, y, tw, th] = w.ribbon.rect_of(RibbonHit::Tab(i)).expect("tab");
    click(w, x + tw / 2.0, y + th / 2.0);
}

#[test]
fn ribbon_is_one_pixel_taller_than_bluedocs() {
    let w = window();
    let mut doc = Ribbon::new();
    doc.layout(&Fixed, &w.strings, 1481.0, 1.25, 0.0);
    assert_eq!(w.ribbon.height() - doc.height(), 1.0);
}

#[test]
fn transitions_tab_sets_duration_and_applies_to_all() {
    let mut w = window();
    tab(&mut w, 3);
    click_command(&mut w, "transition.fade");
    let kind = |w: &SlideWindow| {
        w.view()
            .state()
            .and_then(|s| s.transition.as_ref())
            .map(|t| t.kind.clone())
    };
    assert_eq!(kind(&w).as_deref(), Some("fade"));
    assert!(w.toggled().contains(&"transition.fade"));
    // Choosing it plays it on the canvas, then the canvas is back.
    assert!(w.view().previewing() && w.animating());
    w.set_clock(60_000.0);
    assert!(!w.view().previewing());
    // Duration menu: pick 1 s.
    click_command(&mut w, "transition.duration");
    assert!(w.menu.is_some());
    for _ in 0..3 {
        w.handle(Input::Key(Key::Down, Mods::default()), &Fixed);
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let dur = w
        .view()
        .state()
        .and_then(|s| s.transition.as_ref())
        .map(|t| t.dur_ms);
    assert!(dur.is_some_and(|d| d != 0), "{dur:?}");
    click_command(&mut w, "transition.applyAll");
    w.view_mut().go_to(1);
    assert_eq!(kind(&w).as_deref(), Some("fade"));
    // The gallery's other transitions are in its menu.
    click_command(&mut w, "transition.more");
    assert!(w.menu.is_some());
}

#[test]
fn design_tab_applies_a_theme_and_slide_size() {
    let mut w = window();
    tab(&mut w, 2);
    click_command(&mut w, "design.theme.lotus");
    let theme = w
        .view()
        .state()
        .and_then(|s| s.design.as_ref())
        .and_then(|d| d.theme.clone());
    assert_eq!(theme.as_deref(), Some("BlueOffice Lotus"));
    click_command(&mut w, "design.slideSize");
    assert!(w.menu.is_some());
    w.handle(Input::Key(Key::Down, Mods::default()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let cx = w
        .view()
        .state()
        .and_then(|s| s.design.as_ref())
        .map(|d| d.cx);
    assert!(cx.is_some(), "slide size known");
}

#[test]
fn animations_tab_gives_the_selection_one_effect() {
    let mut w = window();
    w.view_mut().run(SlideCommand::SelectAll);
    let n = w.view().selection().len();
    assert_ne!(n, 0);
    tab(&mut w, 4);
    let count = |w: &SlideWindow| w.view().state().map_or(0, |s| s.animations.len());
    click_command(&mut w, "anim.fade");
    assert_eq!(count(&w), n);
    assert!(w.toggled().contains(&"anim.fade"));
    // Another effect replaces it (the gallery sets, Add Animation adds).
    click_command(&mut w, "anim.flyIn");
    assert_eq!(count(&w), n);
    click_command(&mut w, "anim.none");
    assert_eq!(count(&w), 0);
}

fn drag(w: &mut SlideWindow, from: (f32, f32), to: (f32, f32)) {
    w.handle(
        Input::Press {
            x: from.0,
            y: from.1,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    let steps = 6;
    for k in 1..=steps {
        let t = k as f32 / steps as f32;
        w.handle(
            Input::Drag {
                x: from.0 + (to.0 - from.0) * t,
                y: from.1 + (to.1 - from.1) * t,
            },
            &Fixed,
        );
    }
    w.handle(Input::Release { x: to.0, y: to.1 }, &Fixed);
}

/// Titles of the slides in order (first text of each slide's outline).
fn titles(w: &SlideWindow) -> Vec<String> {
    let p = w.view().engine().presentation().unwrap();
    bo_engine::slide_edit::outline::outline(p)
        .into_iter()
        .map(|s| s.title)
        .collect()
}

#[test]
fn sorter_shows_a_grid_and_drag_reorders() {
    let mut w = window();
    let before = titles(&w);
    // Status bar button: Slide Sorter (reference: 400 × 225 images 46 px apart at 125 %).
    let [x, y, bw, bh] = w.status[7];
    click(&mut w, x + bw / 2.0, y + bh / 2.0);
    assert_eq!(w.view().mode(), ViewMode::Sorter);
    let a = w.view().sort_rect(0);
    let b = w.view().sort_rect(1);
    assert_eq!((a[2], a[3]), (400.0, 225.0));
    assert_eq!(b[0] - a[0] - a[2], 46.0);
    assert_eq!(a[1] - w.ribbon.bottom(), 24.0);
    // Drag slide 1 after slide 3.
    let c = w.view().sort_rect(2);
    drag(
        &mut w,
        (a[0] + 50.0, a[1] + 50.0),
        (c[0] + c[2] - 10.0, c[1] + 50.0),
    );
    let after = titles(&w);
    assert_eq!(after[2], before[0], "{before:?} -> {after:?}");
    assert_eq!(after[0], before[1]);
    // One undo step puts it back.
    w.run_action("edit.undo");
    assert_eq!(titles(&w), before);
    // Double click opens the slide in Normal view.
    let r = w.view().sort_rect(1);
    w.handle(
        Input::Press {
            x: r[0] + 10.0,
            y: r[1] + 10.0,
            count: 2,
            shift: false,
        },
        &Fixed,
    );
    w.handle(
        Input::Release {
            x: r[0] + 10.0,
            y: r[1] + 10.0,
        },
        &Fixed,
    );
    assert_eq!(w.view().mode(), ViewMode::Normal);
    assert_eq!(w.view().current(), 1);
}

#[test]
fn thumbnails_drag_to_reorder_in_normal_view() {
    let mut w = window();
    let before = titles(&w);
    let a = w.view().thumb_rect(0);
    let c = w.view().thumb_rect(2);
    drag(
        &mut w,
        (a[0] + 20.0, a[1] + 20.0),
        (c[0] + 20.0, c[1] + c[3] - 5.0),
    );
    let after = titles(&w);
    assert_eq!(after[2], before[0], "{before:?} -> {after:?}");
}

#[test]
fn reading_view_plays_in_the_window_without_the_ribbon() {
    let mut w = window();
    w.take_requests();
    let [x, y, bw, bh] = w.status[8];
    click(&mut w, x + bw / 2.0, y + bh / 2.0);
    assert_eq!(w.view().mode(), ViewMode::Reading);
    assert!(!w.showing(), "not full screen");
    assert_eq!(w.take_requests().len(), 0);
    assert_eq!(w.view().show_slide(), Some(0));
    w.handle(Input::Key(Key::Right, Mods::default()), &Fixed);
    assert_eq!(w.view().show_slide(), Some(1));
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert_eq!(w.view().mode(), ViewMode::Normal);
    assert_eq!(w.view().current(), 1);
}

#[test]
fn copy_puts_text_and_a_token_on_the_system_clipboard() {
    let mut w = window();
    w.view_mut().run(SlideCommand::SelectAll);
    w.take_requests();
    w.handle(
        Input::Key(
            Key::Letter('c'),
            Mods {
                ctrl: true,
                ..Mods::default()
            },
        ),
        &Fixed,
    );
    let reqs = w.take_requests();
    let Some(Request::SetClipboard { text, internal, .. }) = reqs.first().cloned() else {
        panic!("{reqs:?}");
    };
    assert_ne!(text.len(), 0);
    // Pasting our own copy goes through the engine (shapes keep their formatting).
    let shapes = |w: &SlideWindow| {
        w.view().engine().presentation().unwrap().slides[0]
            .data
            .shapes
            .len()
    };
    let n = shapes(&w);
    assert!(w.paste(Some(internal), None, Some(text)));
    assert!(shapes(&w) > n);
    // Text from another program becomes a text box.
    let n = shapes(&w);
    assert!(w.paste(None, None, Some("Hello".into())));
    assert_eq!(shapes(&w), n + 1);
}

#[test]
fn comments_pane_adds_resolves_and_deletes() {
    let mut w = window();
    let threads = |w: &SlideWindow| {
        w.view()
            .state()
            .map(|s| s.comments.clone())
            .unwrap_or_default()
    };
    let full = w.view().canvas_rect();
    assert!(w.run_action("review.newComment"));
    w.handle(Input::Text("Looks good".into()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let t = threads(&w);
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].comments[0].text, "Looks good");
    // The pane takes room from the editing area.
    assert!(w.view().canvas_rect()[2] < full[2]);
    // Resolve through the pane's link, then delete.
    let hit = |w: &SlideWindow, want: fn(&PaneHit) -> bool| {
        w.comments
            .hits
            .borrow()
            .iter()
            .find(|(_, h)| want(h))
            .map(|(r, _)| *r)
            .expect("link")
    };
    let r = hit(&w, |h| matches!(h, PaneHit::Resolve(..)));
    click(&mut w, r[0] + 2.0, r[1] + 2.0);
    assert!(threads(&w)[0].done);
    let r = hit(&w, |h| matches!(h, PaneHit::Delete(..)));
    click(&mut w, r[0] + 2.0, r[1] + 2.0);
    assert_eq!(threads(&w).len(), 0);
}
