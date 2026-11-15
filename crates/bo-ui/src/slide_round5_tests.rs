//! Tests of BlueSlide round 5: table styles, the Selection Pane and Alt Text (headless).
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

/// Counts filled rectangles.
#[derive(Default)]
struct Count {
    rects: usize,
}
impl Painter for Count {
    fn clear(&mut self, _: bo_render::Rgba) {}
    fn fill_rect(&mut self, _: f32, _: f32, _: f32, _: f32, _: bo_render::Rgba) {
        self.rects += 1;
    }
    fn page(&mut self, _: usize, _: i32, _: i32) {}
    fn text(&mut self, _: &str, _: f32, _: f32, _: TextStyle, _: bo_render::Rgba) {}
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

fn window() -> SlideWindow {
    let mut engine = Engine::with_bundled_fonts();
    let name = "slide-text-bullets.pptx";
    let path = format!(
        "{}/../../tests/corpus/slide/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    engine
        .open(&std::fs::read(path).expect("corpus deck"), name)
        .expect("open");
    let mut w = SlideWindow::new(SlideView::new(engine).expect("view"), "en-US", name);
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
    press(w, x, y, 1);
    w.handle(Input::Release { x, y }, &Fixed);
}

fn press(w: &mut SlideWindow, x: f32, y: f32, count: u8) {
    w.handle(
        Input::Press {
            x,
            y,
            count,
            shift: false,
        },
        &Fixed,
    );
}

fn click_command(w: &mut SlideWindow, id: &'static str) {
    let [x, y, cw, ch] = w.command_rect(id).expect(id);
    click(w, x + cw / 2.0, y + ch / 2.0);
}

fn side_hit(w: &SlideWindow, h: &selpane::SideHit) -> [f32; 4] {
    w.side
        .hits
        .iter()
        .find(|(_, x)| x == h)
        .map_or_else(|| panic!("{h:?}"), |(r, _)| *r)
}

fn table_window() -> SlideWindow {
    let mut w = window();
    w.activate_context = true;
    w.view.run(SlideCommand::InsertTable {
        rows: 3,
        cols: 3,
        rect: None,
    });
    w.handle(Input::Tick, &Fixed);
    w
}

#[test]
fn table_design_gallery_and_style_options() {
    let mut w = table_window();
    assert_eq!(w.ribbon.active_tab(), w.base_tabs, "Table Design");
    for id in fmt::TABLE_TILES {
        assert!(w.command_rect(id).is_some(), "{id}");
    }
    click_command(&mut w, "sl.tbl3");
    let (_, style) = w.table_state().expect("table");
    assert_eq!(style, Some(3));
    let (flags, _) = w.table_state().expect("table");
    assert!(!flags.first_col);
    click_command(&mut w, "sl.firstCol");
    click_command(&mut w, "sl.totalRow");
    let (flags, _) = w.table_state().expect("table");
    assert!(flags.first_col && flags.last_row && flags.first_row);
    // The sixth style from the gallery's "more" menu.
    click_command(&mut w, "sl.tblMore");
    assert!(w.menu.is_some());
    for _ in 0..6 {
        w.handle(Input::Key(Key::Down, Mods::default()), &Fixed);
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(w.table_state().expect("table").1, Some(5));
    // The tiles paint (the gallery box, a framed current tile) without panicking.
    let mut p = Count::default();
    w.paint(&mut p);
    assert!(p.rects > 100);
}

#[test]
fn selection_pane_lists_hides_renames_and_reorders() {
    let mut w = window();
    w.view.run(SlideCommand::InsertShape {
        preset: "rect".into(),
        rect: None,
    });
    let id = w.view().selection()[0].id;
    w.run_action("sl.selPane");
    w.handle(Input::Tick, &Fixed);
    assert!(w.side.shown());
    let rows = w.selection_rows();
    assert_eq!(rows[0].id, id, "front shape first");
    assert!(rows[0].selected);
    // Hide it with its eye: it leaves the selection and the slide.
    let eye = side_hit(&w, &selpane::SideHit::Toggle(id));
    click(&mut w, eye[0] + 4.0, eye[1] + 4.0);
    assert!(w.selection_rows()[0].hidden);
    assert_eq!(w.view().selection().len(), 0);
    // Show All brings it back in one undo step.
    let r = side_hit(&w, &selpane::SideHit::ShowAll);
    click(&mut w, r[0] + 4.0, r[1] + 4.0);
    assert!(!w.selection_rows()[0].hidden);
    // Hide All, then one undo shows every shape again.
    let r = side_hit(&w, &selpane::SideHit::HideAll);
    click(&mut w, r[0] + 4.0, r[1] + 4.0);
    assert!(w.selection_rows().iter().all(|r| r.hidden));
    w.view.run(SlideCommand::Undo);
    assert!(w.selection_rows().iter().all(|r| !r.hidden));
    // Select from the pane, then send it backward.
    w.handle(Input::Tick, &Fixed);
    let r = side_hit(&w, &selpane::SideHit::Select(id));
    click(&mut w, r[0] + 4.0, r[1] + 4.0);
    assert_eq!(w.view().selection()[0].id, id);
    let r = side_hit(&w, &selpane::SideHit::Backward);
    click(&mut w, r[0] + 4.0, r[1] + 4.0);
    assert_eq!(w.selection_rows()[1].id, id);
    // Double click renames in place.
    w.handle(Input::Tick, &Fixed);
    let r = side_hit(&w, &selpane::SideHit::Select(id));
    press(&mut w, r[0] + 4.0, r[1] + 4.0, 2);
    assert!(w.side.rename.is_some());
    for _ in 0..20 {
        w.handle(Input::Key(Key::Backspace, Mods::default()), &Fixed);
    }
    w.handle(Input::Text("Logo".into()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w
        .selection_rows()
        .iter()
        .any(|r| r.id == id && r.name == "Logo"));
}

#[test]
fn alt_text_pane_types_and_marks_decorative() {
    let mut w = window();
    w.activate_context = true;
    w.view.run(SlideCommand::InsertShape {
        preset: "rect".into(),
        rect: None,
    });
    w.handle(Input::Tick, &Fixed);
    click_command(&mut w, "sl.altText");
    w.handle(Input::Tick, &Fixed);
    assert_eq!(w.side.kind, selpane::SideKind::AltText);
    let r = side_hit(&w, &selpane::SideHit::AltBox);
    click(&mut w, r[0] + 10.0, r[1] + 10.0);
    w.handle(Input::Text("A blue box".into()), &Fixed);
    // Leaving the box stores the text.
    click(&mut w, 300.0, 600.0);
    let alt = w.selected_alt();
    assert!(
        alt.as_ref().is_none_or(|(_, t, _)| t == "A blue box"),
        "{alt:?}"
    );
    let id = w.view().selection().first().map(|s| s.id);
    if id.is_none() {
        // The click on the slide cleared the selection: select the box again.
        let rows = w.selection_rows();
        w.view.run(SlideCommand::Select {
            ids: vec![rows[0].id],
        });
    }
    w.handle(Input::Tick, &Fixed);
    assert_eq!(
        w.selected_alt().map(|(_, t, _)| t).as_deref(),
        Some("A blue box")
    );
    let r = side_hit(&w, &selpane::SideHit::Decorative);
    click(&mut w, r[0] + 3.0, r[1] + 3.0);
    assert!(w.selected_alt().is_some_and(|(_, _, d)| d));
}
