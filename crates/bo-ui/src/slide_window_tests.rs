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
    // The chosen layout goes to the current slide (no new slide), as on the reference.
    assert_eq!(w.view().slide_count(), n + 1, "layout changed in place");
    // Delete Slide is not on the reference's Home tab at this width: the command still runs.
    assert!(w.run_action("slide.delete"));
    assert_eq!(w.view().slide_count(), n);
    // Undo from the title bar's quick access button runs through the window.
    assert!(w.run_action("edit.undo"));
    assert_eq!(w.view().slide_count(), n + 1);
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

/// Open ribbon tab `i` (0 Home, 1 Insert, 2 Draw, 3 Design, 4 Transitions, 5 Animations,
/// 6 Slide Show, 7 Review, 8 View).
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
    tab(&mut w, 4);
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
    // Duration spin box: the up arrow adds a quarter second, typing sets it.
    let dur = |w: &SlideWindow| {
        w.view()
            .state()
            .and_then(|s| s.transition.as_ref())
            .map(|t| t.dur_ms)
    };
    let before = dur(&w).unwrap_or(0);
    let [x, y, bw, bh] = w.control_rect("transition.duration").expect("duration box");
    click(&mut w, x + bw - 3.0, y + 3.0);
    assert_eq!(dur(&w), Some(before + 250));
    click(&mut w, x + 5.0, y + bh / 2.0);
    w.handle(Input::Text("1.5".into()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(dur(&w), Some(1500));
    // After: tick it and type 3 s; the slide then advances on its own.
    let [x, y, bw, bh] = w.control_rect("transition.after").expect("after box");
    click(&mut w, x + 5.0, y + bh / 2.0);
    w.handle(Input::Text("3".into()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let after = w
        .view()
        .state()
        .and_then(|s| s.transition.as_ref())
        .and_then(|t| t.advance_after_ms);
    assert_eq!(after, Some(3000));
    assert!(bw > 0.0 && y > 0.0 && x > 0.0);
    // On Mouse Click toggles.
    let [x, y, _, bh] = w.control_rect("transition.onClick").expect("check");
    click(&mut w, x + 3.0, y + bh / 2.0);
    let on_click = w
        .view()
        .state()
        .and_then(|s| s.transition.as_ref())
        .map(|t| t.advance_on_click);
    assert_eq!(on_click, Some(false));
    // Sound: the drop-down offers stopping the previous sound.
    let [x, y, bw, bh] = w.control_rect("transition.sound").expect("sound box");
    click(&mut w, x + bw - 5.0, y + bh / 2.0);
    assert!(w.menu.is_some());
    w.handle(Input::Key(Key::Down, Mods::default()), &Fixed);
    w.handle(Input::Key(Key::Down, Mods::default()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let stop = w
        .view()
        .state()
        .and_then(|s| s.transition.as_ref())
        .and_then(|t| t.sound.as_ref())
        .map(|s| s.stop);
    assert_eq!(stop, Some(true));
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
    tab(&mut w, 3);
    click_command(&mut w, "design.theme.lotus");
    let theme = w
        .view()
        .state()
        .and_then(|s| s.design.as_ref())
        .and_then(|d| d.theme.clone());
    assert_eq!(theme.as_deref(), Some("BlueOffice Lotus"));
    // The fixed-width text measure is wider than the interface font: give the tab room.
    w.handle(
        Input::Resize {
            width: 1900.0,
            height: 1080.0,
        },
        &Fixed,
    );
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
    tab(&mut w, 5);
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
    assert_eq!(a[1] - w.ribbon.bottom(), 25.0);
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
    // Reply in the thread: the reply box under the card.
    let r = hit(&w, |h| matches!(h, PaneHit::Reply(..)));
    click(&mut w, r[0] + 4.0, r[1] + 4.0);
    w.handle(Input::Text("Agreed".into()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let t = threads(&w);
    assert_eq!(t.len(), 1);
    assert_eq!(t[0].comments.len(), 2);
    assert_eq!(t[0].comments[1].text, "Agreed");
    // A marker on the slide opens the pane at its thread.
    let markers = w.comment_markers();
    assert_eq!(markers.len(), 1);
    w.comments.shown = false;
    w.comments.active = None;
    let [mx, my, mw, mh] = markers[0].0;
    click(&mut w, mx + mw / 2.0, my + mh / 2.0);
    assert!(w.comments.shown);
    assert_eq!(w.comments.active.as_deref(), Some(t[0].id.as_str()));
    let r = hit(&w, |h| matches!(h, PaneHit::Resolve(..)));
    click(&mut w, r[0] + 2.0, r[1] + 2.0);
    assert!(threads(&w)[0].done);
    let r = hit(&w, |h| matches!(h, PaneHit::Delete(..)));
    click(&mut w, r[0] + 2.0, r[1] + 2.0);
    assert_eq!(threads(&w).len(), 0);
}

#[test]
fn animation_pane_lists_reorders_and_times_effects() {
    use crate::slide_anim_pane::AnimHit;
    let mut w = window();
    w.view_mut().run(SlideCommand::SelectAll);
    let n = w.view().selection().len();
    assert!(n >= 2, "two shapes to order");
    tab(&mut w, 5);
    click_command(&mut w, "anim.fade");
    click_command(&mut w, "anim.pane");
    assert!(w.anim_pane.shown);
    let names = |w: &SlideWindow| -> Vec<u32> {
        w.view()
            .state()
            .map(|s| s.animations.iter().map(|a| a.shape_id).collect())
            .unwrap_or_default()
    };
    let before = names(&w);
    assert_eq!(before.len(), n);
    let at = |w: &SlideWindow, want: AnimHit| {
        w.anim_pane
            .hits
            .iter()
            .find(|(_, h)| *h == want)
            .map(|(r, _)| *r)
            .expect("pane hit")
    };
    // Choose the second effect, move it earlier.
    let r = at(&w, AnimHit::Select(1));
    click(&mut w, r[0] + 30.0, r[1] + 4.0);
    assert_eq!(w.anim_pane.selected, Some(1));
    let r = at(&w, AnimHit::Earlier);
    click(&mut w, r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    let after = names(&w);
    assert_eq!((after[0], after[1]), (before[1], before[0]));
    assert_eq!(w.anim_pane.selected, Some(0));
    // Timing group: the chosen effect's duration steps up by a quarter second.
    let dur = |w: &SlideWindow| w.view().state().map(|s| s.animations[0].dur_ms);
    let d0 = dur(&w).unwrap_or(0);
    let [x, y, bw, _] = w.control_rect("anim.duration").expect("duration box");
    click(&mut w, x + bw - 3.0, y + 3.0);
    assert_eq!(dur(&w), Some(d0 + 250));
    // Start: With Previous from the drop-down.
    let [x, y, bw, bh] = w.control_rect("anim.start").expect("start box");
    click(&mut w, x + bw - 5.0, y + bh / 2.0);
    assert!(w.menu.is_some());
    // The menu opens on the effect's current start; pick After Previous, then With Previous.
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(w.choose(Choice::Command("trig:afterPrevious".into())));
    let trigger = w.view().state().map(|s| s.animations[0].trigger);
    assert_eq!(trigger, Some("afterPrevious"));
    assert!(w.choose(Choice::Command("trig:withPrevious".into())));
    let trigger = w.view().state().map(|s| s.animations[0].trigger);
    assert_eq!(trigger, Some("withPrevious"));
}

#[test]
fn shapes_paste_into_another_window() {
    let mut a = window();
    let mut b = window();
    a.view_mut().run(SlideCommand::SelectAll);
    a.take_requests();
    assert!(a.run_action("edit.copy"));
    let reqs = a.take_requests();
    let Some(Request::SetClipboard { text, internal, .. }) = reqs.first().cloned() else {
        panic!("{reqs:?}");
    };
    assert!(internal.starts_with("BlueSlide/"));
    let shapes = |w: &SlideWindow| {
        w.view().engine().presentation().unwrap().slides[0]
            .data
            .shapes
            .len()
    };
    let n = shapes(&b);
    let copied = a.view().selection().len();
    // The other window gets the shapes themselves, not a text box.
    assert!(b.paste(Some(internal), None, Some(text)));
    assert_eq!(shapes(&b), n + copied);
}

#[test]
fn draw_tab_pens_and_erases_ink() {
    let mut w = window();
    tab(&mut w, 2);
    click_command(&mut w, "draw.red");
    assert!(w.toggled().contains(&"draw.red"));
    let shapes = |w: &SlideWindow| {
        w.view().engine().presentation().unwrap().slides[0]
            .data
            .shapes
            .len()
    };
    let n = shapes(&w);
    let c = w.view().canvas_rect();
    let (x0, y0) = (c[0] + c[2] * 0.3, c[1] + c[3] * 0.8);
    drag(&mut w, (x0, y0), (x0 + 200.0, y0 - 40.0));
    assert_eq!(shapes(&w), n + 1, "one ink object");
    // The eraser takes it away again.
    click_command(&mut w, "draw.eraser");
    drag(&mut w, (x0 + 100.0, y0 - 60.0), (x0 + 100.0, y0 + 10.0));
    assert_eq!(shapes(&w), n);
    // Select: back to shapes.
    click_command(&mut w, "draw.select");
    assert!(w.view().ink_tool().is_none());
}

#[test]
fn animation_painter_copies_an_effect_to_the_next_shape() {
    let mut w = window();
    w.view_mut().run(SlideCommand::SelectAll);
    let ids: Vec<u32> = w.view().selection().iter().map(|s| s.id).collect();
    assert!(ids.len() >= 2);
    w.view_mut().run(SlideCommand::Select { ids: vec![ids[0]] });
    tab(&mut w, 5);
    click_command(&mut w, "anim.flyIn");
    click_command(&mut w, "anim.painter");
    assert!(w.toggled().contains(&"anim.painter"));
    // Choosing the other shape gives it the same effect.
    w.view_mut().run(SlideCommand::Select { ids: vec![ids[1]] });
    w.handle(Input::Tick, &Fixed);
    let effects: Vec<(u32, String)> = w
        .view()
        .state()
        .map(|s| {
            s.animations
                .iter()
                .map(|a| (a.shape_id, a.effect.clone()))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        effects.contains(&(ids[1], "flyIn".to_owned())),
        "{effects:?}"
    );
    assert!(!w.toggled().contains(&"anim.painter"));
}

#[test]
fn review_steps_through_comments_and_slide_show_hides_slides() {
    let mut w = window();
    // A comment on slide 3, none before it.
    w.view_mut().go_to(2);
    w.view_mut().run(SlideCommand::AddComment {
        text: "Check numbers".into(),
        author: "A".into(),
        date: None,
        parent: None,
        mentions: Vec::new(),
        task: None,
    });
    w.view_mut().go_to(0);
    assert!(w.run_action("review.nextComment"));
    assert_eq!(w.view().current(), 2);
    assert!(w.comments.shown && w.comments.active.is_some());
    assert!(!w.run_action("review.nextComment"), "no later comment");
    assert!(w.run_action("review.deleteComment"));
    assert_eq!(w.view().state().map(|s| s.comments.len()), Some(0));
    // Hide Slide toggles the current slide out of the show.
    assert!(w.run_action("show.hideSlide"));
    let hidden = |w: &SlideWindow| w.view().engine().presentation().unwrap().slides[2].show;
    assert_eq!(hidden(&w), Some(false));
    assert!(w.run_action("show.hideSlide"));
    assert_ne!(hidden(&w), Some(false));
}

fn pick(w: &mut SlideWindow, value: &str) {
    let (x, y) = w
        .picker
        .as_ref()
        .and_then(|(p, _)| p.cell_center(value))
        .expect(value);
    w.handle(Input::Move { x, y }, &Fixed);
    click(w, x, y);
}

fn select_tab(w: &mut SlideWindow, i: usize) {
    let [x, y, tw, th] = w.ribbon.rect_of(RibbonHit::Tab(i)).expect("tab");
    click(w, x + tw / 2.0, y + th / 2.0);
}

#[test]
fn insert_table_from_the_grid_shows_table_tabs() {
    let mut w = window();
    select_tab(&mut w, 1);
    click_command(&mut w, "sl.table");
    assert!(w.picker.is_some(), "table grid");
    pick(&mut w, "table:3x2");
    assert!(w.picker.is_none());
    let sel = w.view().selection();
    assert_eq!(sel.len(), 1);
    assert_eq!(sel[0].kind, "table");
    // Table Design and Layout follow the fixed tabs; the first is active after inserting.
    assert_eq!(w.context, ins::Context::Table);
    assert_eq!(w.ribbon.active_tab(), w.base_tabs);
    assert!(w.command_rect("sl.headerRow").is_some());
    // Without a selection the contextual tabs go and the Insert tab is back.
    w.view.run(SlideCommand::Select { ids: vec![] });
    w.handle(Input::Tick, &Fixed);
    assert_eq!(w.context, ins::Context::None);
    assert_eq!(w.ribbon.active_tab(), 1);
}

#[test]
fn shapes_gallery_inserts_a_preset_and_size_box_steps() {
    let mut w = window();
    select_tab(&mut w, 1);
    click_command(&mut w, "sl.shapes");
    pick(&mut w, "shape:star5");
    assert_eq!(w.context, ins::Context::Shape);
    assert_eq!(w.ribbon.active_tab(), w.base_tabs, "Shape Format");
    let h0 = w.view().selection()[0].rect.h;
    // Up arrow of the Height box: 0.1 inch (7.2 pt) taller.
    let r = w.size_box("sl.height").expect("height box");
    click(&mut w, r[0] + r[2] - 3.0, r[1] + 3.0);
    let h1 = w.view().selection()[0].rect.h;
    assert!((h1 - h0 - 7.2).abs() < 0.2, "{h0} -> {h1}");
    // Typing a width.
    let r = w.size_box("sl.width").expect("width box");
    click(&mut w, r[0] + 5.0, r[1] + r[3] / 2.0);
    w.handle(Input::Text("2".into()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!((w.view().selection()[0].rect.w - 144.0).abs() < 0.5);
    // Shape fill from the palette.
    click_command(&mut w, "sl.fill");
    pick(&mut w, "color:FF0000");
    assert!(w.view.run(SlideCommand::Undo));
}

#[test]
fn find_selects_the_next_match_and_replace_all_counts() {
    let mut w = window();
    let word: String = {
        let p = w.view().engine().presentation().unwrap();
        let shapes = bo_engine::slide_edit::a11y::slide_shapes(p, 0);
        let text = shapes
            .iter()
            .find_map(|s| s.text.clone())
            .expect("some text");
        text.split_whitespace().next().unwrap().to_owned()
    };
    w.run_action("sl.find");
    assert!(w.dialog.is_some());
    w.handle(Input::Text(word.to_lowercase()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let te = w.view().text_edit().expect("match selected");
    let picked: String = String::from_utf16_lossy(
        &te.text.encode_utf16().collect::<Vec<_>>()[te.start.min(te.end)..te.start.max(te.end)],
    );
    assert_eq!(picked.to_lowercase(), word.to_lowercase());
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(w.dialog.is_none());
    assert_eq!(ins::replace_ci(&word, &word, "x").1, 1);
}

#[test]
fn header_and_footer_dialog_applies_slide_numbers() {
    let mut w = window();
    w.run_action("sl.headerFooter");
    let (d, _) = w.dialog.as_mut().expect("dialog");
    // Fields: footer text, then date, slide number, footer, not on title.
    let mut v = d.values();
    assert_eq!(v.len(), 5);
    v[2] = "1".into();
    w.dialog_button(
        ins::SlideDialog::HeaderFooter,
        crate::dialog::DialogButton::Ok,
        &v,
    );
    assert!(w.dialog.is_none());
    w.view.run(SlideCommand::HeaderFooterInfo);
    let info = w
        .view()
        .state()
        .and_then(|s| s.header_footer.clone())
        .unwrap();
    assert!(info.slide_number);
}

#[test]
fn trigger_and_painter_dim_without_an_effect() {
    let w = window();
    let f = ribbon_format(&w.view, false);
    assert!(!slide_enabled("anim.trigger", &f));
    assert!(!slide_enabled("anim.painter", &f));
    assert!(!slide_enabled("sl.fill", &f), "needs a selection");
    let f = ribbon_format(&w.view, true);
    assert!(slide_enabled("anim.trigger", &f));
    assert!(slide_enabled("sl.table", &f));
}

#[test]
fn shows_skip_hidden_slides_loop_and_rehearse() {
    let mut w = window();
    let n = w.view().slide_count();
    assert!(n >= 3, "deck of {n}");
    // Hide slide 2: From Beginning plays 1, 3, …
    w.view_mut().go_to(1);
    assert!(w.run_action("show.hideSlide"));
    let plan = w.view().show_plan(None);
    assert!(!plan.order.contains(&1));
    assert!(w.run_action("show.fromStart"));
    assert!(w.view().showing());
    assert_eq!(w.view().show_slide(), Some(0));
    w.handle(Input::Key(Key::Right, Mods::default()), &Fixed);
    // Effects first if slide 1 has any; then slide 3.
    for _ in 0..10 {
        if w.view().show_slide() != Some(0) {
            break;
        }
        w.handle(Input::Key(Key::Right, Mods::default()), &Fixed);
    }
    assert_eq!(w.view().show_slide(), Some(2), "hidden slide 2 skipped");
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(!w.view().showing());
    // Set Up Slide Show: loop until Esc.
    w.run_action("show.setUp");
    let mut v = w.dialog.as_ref().map(|(d, _)| d.values()).expect("dialog");
    v[2] = "1".into();
    w.dialog_button(
        ins::SlideDialog::Show(showtab::ShowDialog::SetUp),
        crate::dialog::DialogButton::Ok,
        &v,
    );
    let plan = w.view().show_plan(None);
    assert!(plan.looped);
    // Rehearse: the show times each slide; ending it asks to keep the timings.
    w.view_mut().set_clock(0.0);
    assert!(w.run_action("show.rehearse"));
    w.view_mut().set_clock(3_000.0);
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(matches!(
        w.dialog.as_ref().map(|(_, k)| *k),
        Some(ins::SlideDialog::Show(showtab::ShowDialog::KeepTimings))
    ));
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.dialog.is_none());
    let timings = w
        .view()
        .engine()
        .presentation()
        .map(|p| bo_engine::slide_edit::present::present_out(p).timings)
        .unwrap();
    assert_eq!(timings[0], Some(3_000), "{timings:?}");
}

#[test]
fn ending_a_show_with_ink_asks_to_keep_it() {
    let shapes = |w: &SlideWindow| {
        w.view()
            .engine()
            .presentation()
            .map_or(0, |p| p.slides[0].data.shapes.len())
    };
    for keep in [false, true] {
        let mut w = window();
        let before = shapes(&w);
        assert!(w.run_action("show.fromStart"));
        let ctrl = Mods {
            ctrl: true,
            ..Mods::default()
        };
        w.handle(Input::Key(Key::Letter('p'), ctrl), &Fixed);
        let (cx, cy) = (w.size.0 / 2.0, w.size.1 / 2.0);
        w.handle(
            Input::Press {
                x: cx,
                y: cy,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        w.handle(
            Input::Drag {
                x: cx + 80.0,
                y: cy + 30.0,
            },
            &Fixed,
        );
        w.handle(
            Input::Release {
                x: cx + 80.0,
                y: cy + 30.0,
            },
            &Fixed,
        );
        // Esc puts the pen down, a second Esc ends the show.
        w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
        w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
        assert!(!w.view().showing());
        assert_eq!(
            w.dialog.as_ref().map(|(_, k)| *k),
            Some(ins::SlideDialog::KeepInk)
        );
        let key = if keep { Key::Enter } else { Key::Escape };
        w.handle(Input::Key(key, Mods::default()), &Fixed);
        assert!(w.dialog.is_none());
        assert!(!w.view().has_pending_ink());
        assert_eq!(shapes(&w) > before, keep, "keep = {keep}");
    }
}

#[test]
#[allow(clippy::items_after_statements)] // the recording painter belongs to this test
fn presenter_view_shows_the_slide_on_the_left() {
    let mut w = window();
    assert!(w.run_action("show.presenterView"));
    assert!(w.view().presenter());
    struct Texts(Vec<String>);
    impl Painter for Texts {
        fn clear(&mut self, _: bo_render::Rgba) {}
        fn fill_rect(&mut self, _: f32, _: f32, _: f32, _: f32, _: bo_render::Rgba) {}
        fn page(&mut self, _: usize, _: i32, _: i32) {}
        fn text(&mut self, t: &str, _: f32, _: f32, _: TextStyle, _: bo_render::Rgba) {
            self.0.push(t.to_owned());
        }
        fn path(
            &mut self,
            _: &[PathSeg],
            _: Option<bo_render::Rgba>,
            _: Option<(bo_render::Rgba, f32)>,
        ) {
        }
        fn push_clip(&mut self, _: f32, _: f32, _: f32, _: f32) {}
        fn pop_clip(&mut self) {}
    }
    let mut t = Texts(Vec::new());
    w.paint(&mut t);
    assert!(t.0.iter().any(|s| s == "Next Slide"), "{:?}", t.0);
    assert!(t.0.iter().any(|s| s.starts_with("0:00:")), "timer");
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(!w.view().presenter());
}

#[test]
fn view_tab_outline_master_and_grid() {
    let mut w = window();
    assert!(w.run_action("view.outline"));
    w.handle(Input::Tick, &Fixed);
    assert_eq!(w.outline_rows.len(), w.view().slide_count());
    // A press on the second slide's outline row goes there.
    let (top, slide) = w.outline_rows[1];
    let area = w.view().thumbs_area();
    click(&mut w, area[0] + 30.0, top + 4.0);
    assert_eq!(w.view().current(), slide);
    assert!(w.run_action("view.normal"));
    assert!(w.outline_rows.is_empty() || w.pane == viewtab::Pane::Normal);
    // Slide Master: its tab comes first, Close Master View goes back.
    let n = w.base_tabs;
    assert!(w.run_action("view.slideMaster"));
    assert!(w.master_view);
    assert_eq!(w.base_tabs, n + 1);
    assert_eq!(w.ribbon.active_tab(), 0);
    assert!(w.run_action("view.closeMaster"));
    assert!(!w.master_view);
    assert_eq!(w.base_tabs, n);
    // Gridlines and guides toggle.
    assert!(w.run_action("view.gridlines"));
    assert!(w.gridlines);
    assert!(w.run_action("view.notesPage"));
    assert_eq!(w.pane, viewtab::Pane::NotesPage);
}

#[test]
fn lasso_selects_the_shapes_it_surrounds() {
    let mut w = window();
    w.run_action("draw.lasso");
    let c = w.view().canvas_rect();
    // A loop round the whole canvas.
    let pts = [
        (c[0] + 2.0, c[1] + 2.0),
        (c[0] + c[2] - 2.0, c[1] + 2.0),
        (c[0] + c[2] - 2.0, c[1] + c[3] - 2.0),
        (c[0] + 2.0, c[1] + c[3] - 2.0),
        (c[0] + 2.0, c[1] + 4.0),
    ];
    w.handle(
        Input::Press {
            x: pts[0].0,
            y: pts[0].1,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    for (x, y) in &pts[1..] {
        w.handle(Input::Drag { x: *x, y: *y }, &Fixed);
    }
    w.handle(
        Input::Release {
            x: pts[4].0,
            y: pts[4].1,
        },
        &Fixed,
    );
    let n = bo_engine::slide_edit::geom::place_slide(w.view().engine().presentation().unwrap(), 0)
        .len();
    assert!(n > 0);
    assert_eq!(w.view().selection().len(), n);
}

#[test]
fn insert_smartart_text_styles_links_and_menu_marks() {
    let mut w = window();
    assert_eq!(w.choose_insert("smart", "process"), Some(true));
    assert_eq!(w.view().selection().len(), 1);
    assert_eq!(w.choose_insert("tstyle", "1"), Some(true));
    let sel = w.view().selection()[0].clone();
    assert_eq!(sel.text.as_deref(), Some("Your text here"));
    assert_eq!(w.choose_insert("lnk", "next"), Some(true));
    // One-word menu labels keep the mark on a second line, as on the reference.
    let [x, y, tw, th] = w.ribbon.rect_of(RibbonHit::Tab(1)).expect("tab");
    click(&mut w, x + tw / 2.0, y + th / 2.0);
    let lines = w.ribbon.large_lines_of("sl.table");
    assert_eq!(lines.as_ref().map(|l| l[1].0.as_str()), Some("\u{25BE}"));
}
