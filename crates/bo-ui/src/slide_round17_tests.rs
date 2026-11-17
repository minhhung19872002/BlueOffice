//! Round 17 tests of the BlueSlide window: Draw tab pens and ruler, Hide Ink, the effects
//! gallery over the click-number tags, Set Up Show option buttons, Grid and Guides.
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

fn on_tab(w: &mut SlideWindow, i: usize) {
    w.ribbon.set_active_tab(i);
    w.relayout_pending = true;
    w.handle(Input::Blink, &Fixed);
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

#[test]
fn pens_choose_and_second_click_opens_their_menu() {
    let mut w = window();
    on_tab(&mut w, 2);
    assert!(w.action("draw.red"));
    assert_eq!(w.active_pen, Some("draw.red"));
    assert!(matches!(
        w.view.ink_tool(),
        Some(crate::slide_view::InkTool::Pen { color, .. }) if color == "E71224"
    ));
    // The pen in use: its menu (five thicknesses and Pen Color).
    assert!(w.action("draw.red"));
    assert!(w.menu.is_some());
    assert_eq!(w.choose_design("dpen:1:w:1"), Some(true));
    match w.view.ink_tool() {
        Some(crate::slide_view::InkTool::Pen { width, .. }) => {
            assert!((width - 72.0 / 25.4).abs() < 1e-6);
        }
        other => panic!("{other:?}"),
    }
    // A picked colour goes to that pen.
    assert_eq!(w.pick_pen_color("draw.red", "00ff00"), Some(true));
    assert_eq!(w.pens[1].color, "00FF00");
    // Add ▾ > Pencil appends a seventh pen and takes it.
    assert_eq!(w.choose_design("dadd:pencil"), Some(true));
    assert_eq!(w.pens.len(), 7);
    assert_eq!(w.active_pen, Some("draw.pen7"));
    // The ruler toggles.
    assert!(w.action("draw.ruler"));
    assert!(w.view.draw_ruler().is_some());
    assert!(w.action("draw.ruler"));
    assert!(w.view.draw_ruler().is_none());
}

#[test]
fn hide_ink_is_greyed_without_ink_and_toggles_with_it() {
    let mut w = window();
    let f = ribbon_format(&w.view, false);
    assert!(!slide_enabled("sl.hideInk", &f));
    w.view
        .set_ink_tool(crate::slide_view::InkTool::of("draw.pen"));
    let stroke = bo_engine::slide_edit::ink::InkCommand::AddInk {
        stroke: bo_engine::slide_edit::ink::InkStrokeIn {
            points: vec![[100.0, 100.0], [200.0, 150.0]],
            pressure: None,
            times: None,
            color: "000000".into(),
            width: 2.0,
            highlighter: false,
        },
        name: String::new(),
        auto_shape: false,
        attach_to: None,
    };
    let r = w.view.engine_mut().slide_ink_command(stroke).unwrap();
    assert!(r.error.is_none());
    let f = ribbon_format(&w.view, false);
    assert!(slide_enabled("sl.hideInk", &f));
    assert!(slide_enabled("draw.replay", &f));
    assert!(w.choose_review("hideink"));
    assert!(w.view.engine().slide_hide_ink());
    assert!(w.toggled().contains(&"sl.hideInk"));
    assert!(w.choose_review("hideink"));
    assert!(!w.view.engine().slide_hide_ink());
    // Delete All Ink on Slide removes it (Hide Ink greyed again).
    assert!(w.choose_review("delinkslide"));
    assert!(!w.view.engine().slide_has_ink());
}

#[test]
fn effects_gallery_is_drawn_over_the_click_number_tags() {
    let mut w = window();
    let ids: Vec<u32> = w
        .view
        .engine()
        .presentation()
        .map(|p| bo_engine::slide_edit::geom::place_slide(p, 0))
        .unwrap()
        .iter()
        .map(|s| s.id)
        .take(1)
        .collect();
    w.view.run(SlideCommand::Select { ids });
    assert!(w.pick_effect("fade", "entr", true));
    let anim = w.base_tabs - 4;
    on_tab(&mut w, anim);
    assert!(w.anim_tags_shown());
    w.open_effects("anim.none", false);
    assert!(w.picker.is_some());
    assert!(!w.anim_tags_shown(), "tags hidden under the open gallery");
    w.picker = None;
    assert!(w.anim_tags_shown());
}

#[test]
fn set_up_show_option_buttons_are_exclusive_and_saved() {
    let mut w = window();
    assert!(w.open_form8(forms8::Dlg::SetUp));
    w.handle(Input::Blink, &Fixed);
    {
        let o = w.form8.as_mut().unwrap();
        assert!(o.form.on("t_speaker") && !o.form.on("t_kiosk"));
        // Defaults as the reference: narrations play, presenter view on.
        assert!(!o.form.on("nonarr"));
        assert!(o.form.on("presenter"));
        o.form.set_on("t_kiosk", true);
    }
    assert!(w.form8_event(&sform::Event::Changed("t_kiosk")));
    {
        let o = w.form8.as_ref().unwrap();
        assert!(o.form.on("t_kiosk") && !o.form.on("t_speaker") && !o.form.on("t_browse"));
    }
    assert!(w.form8_event(&sform::Event::Ok));
    let setup = w
        .view
        .engine()
        .presentation()
        .map(bo_engine::slide_edit::present::show_setup)
        .unwrap();
    assert_eq!(
        setup.show_type,
        bo_engine::slide_edit::present::ShowType::Kiosk
    );
}

#[test]
fn grid_dialog_sets_snapping_and_spacing() {
    let mut w = window();
    assert!(w.open_form8(forms8::Dlg::Grid));
    {
        let o = w.form8.as_mut().unwrap();
        o.form.set_on("snap", true);
        o.form.set_sel("spacing", Some(3));
    }
    assert!(w.form8_event(&sform::Event::Ok));
    assert_eq!(w.view.grid_snap(), Some(36.0));
}

#[test]
fn thesaurus_pane_lists_meanings_looks_up_and_goes_back() {
    let mut w = window();
    let f = ribbon_format(&w.view, false);
    assert!(slide_enabled("sl.thesaurus", &f));
    w.side.thes.word = "increase".into();
    assert!(w.action("sl.thesaurus"));
    assert_eq!(w.side.kind, selpane::SideKind::Thesaurus);
    // Senses with the same heading are merged; verbs come with their antonym.
    let m = w.thesaurus_meanings();
    let heads: Vec<_> = m
        .iter()
        .map(|x| (x.pos.clone(), x.synonyms.first().cloned()))
        .collect();
    let mut uniq = heads.clone();
    uniq.dedup();
    assert_eq!(heads.len(), uniq.len(), "{heads:?}");
    assert!(m.iter().any(|x| x.pos == "v"), "{heads:?}");
    assert!(m.iter().any(|x| x.antonyms.iter().any(|a| a == "decrease")));
    w.handle(Input::Blink, &Fixed);
    let syn = w
        .side
        .hits
        .iter()
        .find_map(|(_, h)| match h {
            selpane::SideHit::ThesLookup(s) if s != "increase" => Some(s.clone()),
            _ => None,
        })
        .expect("a synonym row");
    assert!(w.thesaurus_hit(&selpane::SideHit::ThesLookup(syn.clone()), 1));
    assert_eq!(w.side.thes.word, syn);
    assert!(w.thesaurus_hit(&selpane::SideHit::ThesBack, 1));
    assert_eq!(w.side.thes.word, "increase");
    // A heading folds its meaning.
    assert!(w.thesaurus_hit(&selpane::SideHit::ThesToggle(0), 1));
    assert_eq!(w.side.thes.folded, vec![0]);
    // Without text editing a double press inserts nothing.
    assert!(!w.thesaurus_hit(&selpane::SideHit::ThesLookup(syn), 2));
}

#[test]
fn animation_pane_follows_the_slide_selection() {
    let mut engine = Engine::with_bundled_fonts();
    let name = "slide-edit-animations-all.pptx";
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
    // The first animated slide.
    for i in 0..w.view.slide_count() {
        w.view.go_to(i);
        if w.view.state().is_some_and(|s| !s.animations.is_empty()) {
            break;
        }
    }
    let Some(id) = w
        .view
        .state()
        .and_then(|s| s.animations.last().map(|a| a.shape_id))
    else {
        panic!("the deck has an animated slide");
    };
    w.anim_pane.shown = true;
    w.view.run(SlideCommand::Select { ids: vec![id] });
    w.relayout_pending = true;
    w.handle(Input::Blink, &Fixed);
    let sel = w
        .anim_pane
        .selected
        .expect("the selected shape's row is chosen");
    assert_eq!(w.view.state().unwrap().animations[sel].shape_id, id);
}
