//! Round 21 tests of the BlueSlide window: the slide show's right-click menu with the
//! reference's rows and cascading submenus, and See All Slides.
use super::*;
use bo_engine::Engine;
use bo_render::PageList;

struct Fixed;
impl TextMeasure for Fixed {
    fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
        (
            text.chars().count() as f32 * style.size * 0.55,
            style.size * 1.33,
        )
    }
}

/// A page store that remembers what it was asked to draw.
#[derive(Default)]
struct Store {
    drawn: Vec<(usize, f32, f32)>,
}

impl PageStore for Store {
    fn update(
        &mut self,
        index: usize,
        list: &PageList,
        _engine: &Engine,
        scale: f32,
        _band: Option<[u32; 4]>,
    ) -> Result<(), String> {
        self.drawn
            .push((index, list.width * scale, list.height * scale));
        Ok(())
    }

    fn remove(&mut self, _index: usize) {}
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

/// Records the texts painted and the pages drawn.
#[derive(Default)]
struct Rec {
    texts: Vec<String>,
    pages: Vec<(usize, i32, i32)>,
}

impl Painter for Rec {
    fn clear(&mut self, _: bo_render::Rgba) {}
    fn fill_rect(&mut self, _: f32, _: f32, _: f32, _: f32, _: bo_render::Rgba) {}
    fn page(&mut self, i: usize, x: i32, y: i32) {
        self.pages.push((i, x, y));
    }
    fn text(&mut self, t: &str, _: f32, _: f32, _: TextStyle, _: bo_render::Rgba) {
        self.texts.push(t.to_owned());
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

fn labels(w: &SlideWindow) -> Vec<String> {
    w.menu
        .as_ref()
        .map(|m| m.items().iter().map(|i| i.label.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn show_menu_has_the_reference_rows_and_cascades() {
    let mut w = window();
    assert!(w.view.start_show(0));
    w.handle(Input::Context { x: 400.0, y: 300.0 }, &Fixed);
    assert_eq!(
        labels(&w),
        [
            "Next",
            "Previous",
            "Last Viewed",
            "See All Slides",
            "Zoom In",
            "Custom Show",
            "Show Presenter View",
            "Screen",
            "Pointer Options",
            "Pause",
            "End Show"
        ]
    );
    let menu = w.menu.as_ref().expect("menu");
    // At the first slide Previous is grey; nothing to pause.
    assert!(menu.items()[1].dim && menu.items()[9].dim);
    assert!(!menu.items()[0].dim);
    // Pointing at Pointer Options opens its submenu beside the row.
    let row = menu.row_rect(8);
    w.handle(
        Input::Move {
            x: row[0] + 20.0,
            y: row[1] + row[3] / 2.0,
        },
        &Fixed,
    );
    let (r, sub) = w.show_sub.as_ref().expect("submenu");
    assert_eq!(*r, 8);
    let first = sub.row_rect(0);
    assert!(first[0] >= row[0] + row[2] - 10.0, "beside the menu");
    let pen = sub.row_rect(1);
    assert_eq!(sub.items()[1].label, "Pen");
    // The arrow is ticked.
    assert!(sub.items()[5].checked);
    w.handle(
        Input::Move {
            x: pen[0] + 20.0,
            y: pen[1] + pen[3] / 2.0,
        },
        &Fixed,
    );
    assert!(w.show_sub.is_some(), "moving into the submenu keeps it");
    w.handle(
        Input::Press {
            x: pen[0] + 20.0,
            y: pen[1] + pen[3] / 2.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(
        Input::Release {
            x: pen[0] + 20.0,
            y: pen[1] + pen[3] / 2.0,
        },
        &Fixed,
    );
    assert!(w.menu.is_none() && w.show_sub.is_none());
    assert_eq!(
        w.view.show_tools().pointer,
        crate::slide_view::show_tools::Pointer::Pen
    );
    // Screen › Black Screen by keys: Down to Screen, Right, Enter.
    w.handle(Input::Context { x: 400.0, y: 300.0 }, &Fixed);
    for _ in 0..8 {
        w.handle(Input::Key(Key::Down, Mods::default()), &Fixed);
    }
    assert_eq!(w.menu.as_ref().and_then(Menu::hovered), Some(7));
    w.handle(Input::Key(Key::Right, Mods::default()), &Fixed);
    assert!(w.show_sub.as_ref().is_some_and(|(r, _)| *r == 7));
    w.handle(Input::Key(Key::Down, Mods::default()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.menu.is_none());
    assert!(w.view.show_tools().screen.is_some());
    // The submenu arrows are painted with the menu.
    let mut rec = Rec::default();
    w.handle(Input::Context { x: 400.0, y: 300.0 }, &Fixed);
    w.paint(&mut rec);
    assert!(rec.texts.iter().any(|t| t == "Pointer Options"));
}

#[test]
fn see_all_slides_grid_jumps_and_goes_back() {
    let mut w = window();
    assert!(w.view.start_show(0));
    let n = w.view.slide_count();
    assert!(n >= 2);
    // G opens the grid on the slide shown.
    w.handle(Input::Text("g".into()), &Fixed);
    let o = w.view.show_overview().expect("grid");
    assert_eq!(o.sel, 0);
    // Images at a sixth of the screen's width, drawn for the rows in view.
    let mut store = Store::default();
    w.prepare(&mut store).unwrap();
    let r0 = w.view.overview_rect(0);
    assert!(
        (r0[2] - (1481.0f32 / 1920.0 * 320.0).round()).abs() <= 1.0,
        "{r0:?}"
    );
    let thumbs: Vec<_> = store.drawn.iter().filter(|d| d.0 >= 1 << 40).collect();
    assert_eq!(thumbs.len(), n, "{:?}", store.drawn);
    assert!((thumbs[0].1 - r0[2]).abs() <= 1.0);
    let mut rec = Rec::default();
    w.paint(&mut rec);
    assert_eq!(rec.pages.len(), n);
    assert!(rec.texts.iter().any(|t| t == "2"));
    // Right then Enter shows slide 2 and closes the grid.
    w.handle(Input::Key(Key::Right, Mods::default()), &Fixed);
    assert_eq!(w.view.show_overview().map(|o| o.sel), Some(1));
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.view.show_overview().is_none());
    assert_eq!(w.view.show_slide(), Some(1));
    // From the menu; a click on the first image goes there.
    assert!(w.choose(Choice::Command("shw:all".into())));
    let r = w.view.overview_rect(0);
    w.handle(
        Input::Press {
            x: r[0] + 5.0,
            y: r[1] + 5.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    assert_eq!(w.view.show_slide(), Some(0));
    assert!(w.view.show_overview().is_none());
    // Esc closes the grid without ending the show.
    w.handle(Input::Text("-".into()), &Fixed);
    assert!(w.view.show_overview().is_some());
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(w.view.show_overview().is_none());
    assert!(w.view.showing());
}
