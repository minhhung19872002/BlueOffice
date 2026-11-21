//! Round 22 of BlueSlide: Format Background in the window (picture / texture fill, the
//! picked file as background, sync with the slide), Pointer Options › Ink Color and Arrow
//! Options, Custom Show › in the show menu.
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
    engine
        .open(&std::fs::read(path).expect("corpus"), name)
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

fn background(w: &SlideWindow) -> Option<bo_model::slide::Fill> {
    let p = w.view.engine().presentation()?;
    match &p.slides.get(w.view.current())?.data.background {
        Some(bo_model::slide::Background::Props { fill, .. }) => Some(fill.clone()),
        _ => None,
    }
}

#[test]
fn format_background_texture_and_inserted_picture() {
    let mut w = window();
    assert!(w.choose(Choice::Command("ins:formatBg".into())));
    w.relayout(&Fixed);
    // The pane is as wide as the reference's (357 px at 125 %).
    assert!(w.bg_pane.contains(w.size.0 - 300.0, 400.0));
    assert!(!w.bg_pane.contains(w.size.0 - 360.0, 400.0));
    // Picture or texture fill: the first texture, tiled.
    w.bg_pane.kind = 0;
    w.bg_pane.synced = Some(w.view.current());
    w.bg_pane.pending = crate::slide_textures22::png(3)
        .map(|b| base64::Engine::encode(&base64::engine::general_purpose::STANDARD, b));
    w.bg_pane.kind = 2;
    assert!(w.run_bg_current(false));
    assert!(matches!(
        background(&w),
        Some(bo_model::slide::Fill::Picture(b)) if b.tile.is_some()
    ));
    assert!(w.bg_pane.pic.has_picture);
    // Insert…: the next picked file becomes the background, stretched.
    w.bg_pane.awaiting_file = true;
    let png = crate::slide_textures22::png(5).expect("png");
    assert!(w.insert_file(png, "photo.png"));
    assert!(!w.bg_pane.awaiting_file);
    assert!(matches!(
        background(&w),
        Some(bo_model::slide::Fill::Picture(b)) if b.tile.is_none() && b.stretch
    ));
    // Another slide: the pane reads that slide's (inherited) background on the next input.
    let n = w.view.engine().presentation().map_or(0, |p| p.slides.len());
    if n > 1 {
        w.view.go_to(1);
        w.handle(Input::Move { x: 5.0, y: 5.0 }, &Fixed);
        assert_eq!(w.bg_pane.synced, Some(1));
        assert_eq!(w.bg_pane.kind, 0);
    }
}

#[test]
fn ink_color_and_arrow_options_in_the_show() {
    let mut w = window();
    assert!(w.view.start_show(0));
    assert!(w.view.show_command("ink:0070C0"));
    let t = w.view.show_tools();
    assert_eq!(t.ink, Some(bo_render::Rgba(0x00, 0x70, 0xc0, 255)));
    assert_eq!(
        t.pointer,
        crate::slide_view::show_tools::Pointer::Pen,
        "choosing an ink takes the pen"
    );
    assert!(w.view.show_command("arrowHidden"));
    assert_eq!(w.view.show_tools().arrow_mode, 1);
    assert!(w.view.show_command("arrowAuto"));
    assert_eq!(w.view.show_tools().arrow_mode, 2);
    // The menu: Pointer Options › Ink Color › opens beside its row.
    w.handle(Input::Context { x: 400.0, y: 300.0 }, &Fixed);
    let row = w.menu.as_ref().expect("menu").row_rect(8);
    w.handle(
        Input::Move {
            x: row[0] + 20.0,
            y: row[1] + row[3] / 2.0,
        },
        &Fixed,
    );
    let ink_row = w.show_sub.as_ref().expect("submenu").1.row_rect(3);
    w.handle(
        Input::Move {
            x: ink_row[0] + 20.0,
            y: ink_row[1] + ink_row[3] / 2.0,
        },
        &Fixed,
    );
    let (r, sub2) = w.show_sub2.as_ref().expect("ink colours");
    assert_eq!(*r, 3);
    assert_eq!(sub2.items().len(), 12);
    assert!(sub2.items()[7].checked, "blue is the current ink");
}
