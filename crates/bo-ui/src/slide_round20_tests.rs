//! Round 20 tests of the BlueSlide window (round 10 of the slide series): the File screen
//! (Print preview, settings menus, Ctrl+P, Escape) and the slide show's right-click menu and
//! eraser.
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

const PREVIEW: usize = 13 << 40;

#[test]
fn ctrl_p_opens_print_with_a_live_preview() {
    let mut w = window();
    w.set_printers(vec!["Printer A".into(), "Printer B".into()], 1);
    w.handle(
        Input::Key(
            Key::Letter('p'),
            Mods {
                ctrl: true,
                ..Mods::default()
            },
        ),
        &Fixed,
    );
    assert!(w.file_screen_open());
    let b = w.f10.bs.as_ref().expect("file screen");
    assert_eq!(b.page, file10::BsPage::Print);
    assert_eq!(b.print.printer, 1);
    let mut store = Store::default();
    w.prepare(&mut store).unwrap();
    let (_, r) = w
        .f10
        .bs
        .as_ref()
        .and_then(file10::SlideBackstage::preview_rect)
        .expect("preview");
    let drawn = store
        .drawn
        .iter()
        .find(|(i, _, _)| *i == PREVIEW)
        .expect("preview drawn");
    // The slide image fills the preview rectangle.
    assert!((drawn.1 - r[2]).abs() < 1.0, "{drawn:?} {r:?}");
    // Drawn once: the next frame keeps it.
    let mut again = Store::default();
    w.prepare(&mut again).unwrap();
    assert!(!again.drawn.iter().any(|(i, _, _)| *i == PREVIEW));
    // Handouts, 3 per page: a portrait page is drawn.
    assert!(w.choose_file10("layout:4"));
    w.prepare(&mut again).unwrap();
    let (_, pw, ph) = *again
        .drawn
        .iter()
        .find(|(i, _, _)| *i == PREVIEW)
        .expect("handout preview");
    assert!(ph > pw);
    // Escape leaves; the settings are kept for the next time.
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(!w.file_screen_open());
    assert!(w.open_file_screen(file10::BsPage::Print));
    assert_eq!(w.f10.bs.as_ref().map(|b| b.print.layout), Some(4));
}

#[test]
fn print_job_and_export_requests() {
    let mut w = window();
    w.set_printers(vec!["Microsoft Print to PDF".into()], 0);
    w.open_file_screen(file10::BsPage::Print);
    assert!(w.choose_file10("range:1"));
    assert!(w.choose_file10("color:1"));
    assert!(w.file10_out(file10::BsOut::Print));
    let reqs = w.take_file_requests();
    let [FileRequest::Print(job)] = reqs.as_slice() else {
        panic!("{reqs:?}");
    };
    assert_eq!(job.printer, "Microsoft Print to PDF");
    assert_eq!(job.slides, vec![w.view.current()]);
    assert_eq!(job.color, 1);
    assert!(w.file10_out(file10::BsOut::ExportHandouts));
    assert_eq!(w.take_file_requests(), vec![FileRequest::ExportHandouts(6)]);
    assert!(w.file10_out(file10::BsOut::New(Some("ember"))));
    assert_eq!(
        w.take_file_requests(),
        vec![FileRequest::New(Some("ember".into()))]
    );
}

#[test]
fn show_right_click_menu_and_eraser() {
    let mut w = window();
    assert!(w.view.start_show(0));
    w.handle(Input::Context { x: 400.0, y: 300.0 }, &Fixed);
    assert!(w.menu.is_some(), "the right click opens the show menu");
    assert!(w.choose(Choice::Command("shw:black".into())));
    w.menu = None;
    assert!(w.view.show_tools().screen.is_some());
    // Next, then Last Viewed goes back.
    w.handle(Input::Key(Key::Letter('x'), Mods::default()), &Fixed);
    let first = w.view.show_slide();
    assert!(w.view.show_command("next"));
    while w.view.show_slide() == first && w.view.show_command("next") {}
    assert_ne!(w.view.show_slide(), first);
    assert!(w.view.show_command("last"));
    assert_eq!(w.view.show_slide(), first);
    // Pen stroke, then the eraser removes it.
    let r = w.view.show_slide_rect();
    let (x, y) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
    assert!(w.view.show_command("pen"));
    w.handle(
        Input::Press {
            x,
            y,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Drag { x: x + 40.0, y }, &Fixed);
    w.handle(Input::Release { x: x + 40.0, y }, &Fixed);
    let i = w.view.show_slide().expect("slide");
    assert_eq!(w.view.show_tools().strokes.get(&i).map(Vec::len), Some(1));
    w.handle(
        Input::Key(
            Key::Letter('e'),
            Mods {
                ctrl: true,
                ..Mods::default()
            },
        ),
        &Fixed,
    );
    w.handle(
        Input::Press {
            x: x + 20.0,
            y: y + 3.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    assert_eq!(w.view.show_tools().strokes.get(&i).map(Vec::len), Some(0));
}

#[test]
fn slide_size_asks_maximize_or_ensure_fit() {
    let mut w = window();
    assert!(w.choose(Choice::Command("size:43".into())));
    let kind = w.dialog.as_ref().map(|(_, k)| *k);
    assert!(matches!(
        kind,
        Some(ins::SlideDialog::ScaleContent { cx: 9_144_000, .. })
    ));
    w.dialog_button(kind.unwrap(), crate::dialog::DialogButton::Replace, &[]);
    assert!(w.dialog.is_none());
    let d = w
        .view
        .state()
        .and_then(|s| s.design.clone())
        .expect("design");
    assert_eq!((d.cx, d.cy), (9_144_000, 6_858_000));
    // The same size again asks nothing.
    assert!(!w.choose(Choice::Command("size:43".into())));
    assert!(w.dialog.is_none());
}

#[test]
fn format_background_gradient_and_pattern() {
    let mut w = window();
    w.bg_pane.shown = true;
    w.handle(Input::Tick, &Fixed);
    w.relayout(&Fixed);
    w.bg_pane.kind = 1;
    w.bg_pane.grad.angle = 0.0;
    assert!(w.run_bg_current(false));
    let slide = w.view.current();
    let p = w.view.engine().presentation().expect("deck");
    assert!(matches!(
        &p.slides[slide].data.background,
        Some(bo_model::slide::Background::Props {
            fill: bo_model::slide::Fill::Gradient(_),
            ..
        })
    ));
}
