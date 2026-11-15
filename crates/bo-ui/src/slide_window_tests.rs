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
