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
            "Magnify Slide",
            "Custom Show",
            "Show Presenter View",
            "Screen",
            "Pointer Options",
            "Help",
            "Pause",
            "End Show"
        ]
    );
    let menu = w.menu.as_ref().expect("menu");
    // At the first slide Previous is grey; nothing to pause.
    assert!(menu.items()[1].dim && menu.items()[10].dim && !menu.items()[4].dim);
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

#[test]
fn info_dates_are_local_short_dates() {
    use super::file10_win::local_short_date;
    // UTC midnight seen from UTC+7 (reference: "11/7/2026 7:00 AM").
    assert_eq!(
        local_short_date("2026-11-07T00:00:00Z", 420),
        "11/7/2026 7:00 AM"
    );
    assert_eq!(
        local_short_date("2026-12-31T20:30:00Z", 420),
        "1/1/2027 3:30 AM"
    );
    assert_eq!(
        local_short_date("2026-03-01T12:05:00+07:00", 0),
        "3/1/2026 5:05 AM"
    );
    assert_eq!(local_short_date("", 0), "");
}

#[test]
fn print_page_has_printer_properties_and_zoom() {
    let mut w = window();
    w.set_printers(vec!["Printer A".into()], 0);
    w.open_file_screen(file10::BsPage::Print);
    w.handle(Input::Move { x: 5.0, y: 5.0 }, &Fixed);
    let b = w.f10.bs.as_ref().expect("file screen");
    let z = b.preview_zoom().expect("zoom");
    assert!((20..=100).contains(&z), "{z}");
    let _ = w.file10_out(file10::BsOut::PrinterProperties);
    assert_eq!(
        w.take_file_requests(),
        vec![FileRequest::PrinterProperties("Printer A".into())]
    );
}

#[test]
fn background_styles_gallery_applies_to_every_slide() {
    let mut w = window();
    let i = w
        .ribbon
        .tab_index("slideNative.design")
        .expect("design tab");
    let [x, y, tw, th] = w
        .ribbon
        .rect_of(crate::ribbon::RibbonHit::Tab(i))
        .expect("design tab");
    for input in [
        Input::Press {
            x: x + tw / 2.0,
            y: y + th / 2.0,
            count: 1,
            shift: false,
        },
        Input::Release {
            x: x + tw / 2.0,
            y: y + th / 2.0,
        },
    ] {
        w.handle(input, &Fixed);
    }
    assert!(w.open_bg_styles());
    let g = w.bgs.as_ref().expect("gallery");
    assert_eq!(g.tiles.len(), 12);
    // 505 x 370 device px at 125 %.
    assert!(
        (g.rect[2] - 505.0).abs() <= 1.0 && (g.rect[3] - 370.0).abs() <= 1.0,
        "{:?}",
        g.rect
    );
    assert!(matches!(g.tiles[11].1, bgstyles::BgStyle::Gradient(..)));
    // No background of its own: Reset is grey and does nothing; a tile is current.
    assert!(!g.own && g.current.is_some());
    let reset = g.rows[1];
    w.handle(
        Input::Release {
            x: reset[0] + 20.0,
            y: reset[1] + 5.0,
        },
        &Fixed,
    );
    assert!(w.bgs.is_some());
    assert!(
        w.bgs.as_ref().is_some_and(|g| g.marks.is_some()),
        "mnemonics measured"
    );
    let g = w.bgs.as_ref().expect("gallery");
    let modified = w.view.modified();
    let r = g.tiles[11].0;
    let (cx, cy) = (r[0] + 4.0, r[1] + 4.0);
    w.handle(Input::Move { x: cx, y: cy }, &Fixed);
    assert_eq!(w.bgs.as_ref().and_then(|g| g.hover), Some(11));
    w.handle(Input::Release { x: cx, y: cy }, &Fixed);
    assert!(w.bgs.is_none());
    assert!(w.view.modified() && !modified);
}

#[test]
fn see_all_slides_zoom_bar_changes_the_image_size() {
    let mut w = window();
    assert!(w.view.start_show(0));
    assert!(w.view.open_show_overview());
    let before = w.view.overview_rect(0)[2];
    assert!(w.view.step_overview_zoom(1));
    assert!(w.view.overview_rect(0)[2] > before);
    assert!(w.view.step_overview_zoom(-1) && w.view.step_overview_zoom(-1));
    assert!(w.view.overview_rect(0)[2] < before);
}

#[test]
fn magnify_slide_zooms_pans_and_goes_back() {
    let mut w = window();
    assert!(w.view.start_show(0));
    assert!(w.choose(Choice::Command("shw:magnify".into())));
    let r = w.view.show_slide_rect();
    w.handle(
        Input::Move {
            x: r[0] + 10.0,
            y: r[1] + 10.0,
        },
        &Fixed,
    );
    w.handle(
        Input::Press {
            x: r[0] + 10.0,
            y: r[1] + 10.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    let m = w.view.show_magnify().expect("magnifying");
    assert_eq!(m.center, Some((0.25, 0.25)), "clamped to the slide");
    // The twice-as-large image is drawn.
    let mut store = Store::default();
    w.prepare(&mut store).unwrap();
    assert!(store
        .drawn
        .iter()
        .any(|d| d.0 >> 40 == 22 && (d.1 - 2.0 * r[2]).abs() <= 2.0));
    // Dragging left pans right; the slide does not change on a click.
    let shown = w.view.show_slide();
    w.handle(
        Input::Press {
            x: r[0] + 10.0,
            y: r[1] + 10.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(
        Input::Drag {
            x: r[0] - 90.0,
            y: r[1] + 10.0,
        },
        &Fixed,
    );
    w.handle(
        Input::Release {
            x: r[0] - 90.0,
            y: r[1] + 10.0,
        },
        &Fixed,
    );
    assert!(w
        .view
        .show_magnify()
        .and_then(|m| m.center)
        .is_some_and(|c| c.0 > 0.25));
    assert_eq!(w.view.show_slide(), shown);
    // A right click goes back to the whole slide (no menu).
    w.handle(Input::Context { x: 400.0, y: 300.0 }, &Fixed);
    assert!(w.view.show_magnify().is_none() && w.menu.is_none());
    // "+" zooms into the middle; Esc goes back.
    w.handle(Input::Text("+".into()), &Fixed);
    let m = w.view.show_magnify().expect("zoomed");
    assert_eq!(m.level, 1);
    assert!(
        (m.factor() - 1.26).abs() < 0.01,
        "a 2^(1/3) step as measured"
    );
    w.handle(Input::Text("+".into()), &Fixed);
    w.handle(Input::Text("-".into()), &Fixed);
    assert_eq!(w.view.show_magnify().map(|m| m.level), Some(1));
    assert_eq!(
        w.view.show_magnify().and_then(|m| m.center),
        Some((0.5, 0.5))
    );
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(w.view.show_magnify().is_none() && w.view.showing());
    // F1 shows Help too.
    w.handle(Input::Key(Key::F(1), Mods::default()), &Fixed);
    assert!(w.show_help);
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    // Help shows the keys until a key is pressed.
    assert!(w.choose(Choice::Command("shw:help".into())));
    assert!(w.show_help);
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(!w.show_help && w.view.showing());
}

#[test]
fn show_menu_access_keys() {
    let mut w = window();
    assert!(w.view.start_show(0));
    w.handle(Input::Context { x: 400.0, y: 300.0 }, &Fixed);
    let menu = w.menu.as_ref().expect("menu");
    assert!(menu.has_access_keys());
    assert_eq!(menu.items()[3].label, "See All Slides", "markers taken out");
    // "c" opens Screen ›, then "b" blacks the screen.
    w.handle(Input::Text("c".into()), &Fixed);
    assert!(w.show_sub.is_some());
    w.handle(Input::Text("b".into()), &Fixed);
    assert!(w.menu.is_none() && w.view.show_tools().screen.is_some());
    // "a" runs See All Slides.
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    w.handle(Input::Context { x: 400.0, y: 300.0 }, &Fixed);
    w.handle(Input::Text("a".into()), &Fixed);
    assert!(w.view.show_overview().is_some());
}

#[test]
fn show_help_dialog_tabs_and_ok() {
    let mut w = window();
    assert!(w.view.start_show(0));
    w.handle(Input::Key(Key::F(1), Mods::default()), &Fixed);
    assert!(w.show_help);
    let tabs = w.help_tabs();
    assert_eq!(tabs.len(), 5);
    let r = tabs[3];
    w.handle(
        Input::Press {
            x: r[0] + 4.0,
            y: r[1] + 4.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    assert_eq!(w.help_tab, 3);
    // Keys do not reach the show while Help is open.
    let shown = w.view.show_slide();
    w.handle(Input::Key(Key::Right, Mods::default()), &Fixed);
    assert_eq!(w.view.show_slide(), shown);
    assert_eq!(w.help_tab, 4);
    let ok = w.help_ok();
    w.handle(
        Input::Press {
            x: ok[0] + 4.0,
            y: ok[1] + 4.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    assert!(!w.show_help && w.view.showing());
    // Ctrl+S: all slides.
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
    assert!(w.view.show_overview().is_some());
}

#[test]
fn show_pointer_hiding_and_taskbar() {
    let mut w = window();
    assert!(w.view.start_show(0));
    let ctrl = |c| {
        Input::Key(
            Key::Letter(c),
            Mods {
                ctrl: true,
                ..Mods::default()
            },
        )
    };
    w.view.set_clock(1000.0);
    assert!(!w.view.show_cursor_hidden());
    w.handle(ctrl('h'), &Fixed);
    assert!(w.view.show_cursor_hidden(), "Ctrl+H hides the pointer");
    w.handle(ctrl('a'), &Fixed);
    assert!(!w.view.show_cursor_hidden(), "Ctrl+A shows it");
    // Ctrl+U: hidden only after 3 s without a move.
    w.handle(ctrl('u'), &Fixed);
    assert!(!w.view.show_cursor_hidden());
    let now = w.view.clock_ms();
    w.view.set_clock(now + 3500.0);
    assert!(w.view.show_cursor_hidden());
    w.handle(Input::Move { x: 10.0, y: 10.0 }, &Fixed);
    assert!(!w.view.show_cursor_hidden());
    // Ctrl+T leaves full screen; the next key returns without acting.
    let _ = w.take_requests();
    w.handle(ctrl('t'), &Fixed);
    assert!(w.take_requests().contains(&Request::FullScreen(false)));
    w.handle(Input::Key(Key::Home, Mods::default()), &Fixed);
    assert!(w.take_requests().contains(&Request::FullScreen(true)));
    assert!(w.view.showing());
}
