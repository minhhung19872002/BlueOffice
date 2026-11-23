//! BlueDoc parity round 18: window commands and the Screenshot gallery.
use super::super::{DocWindow, Request};
use crate::{DocView, Input, Painter, TextMeasure, TextStyle};
use bo_engine::Engine;
use bo_render::{PathSeg, Rgba};

struct Fixed;
impl TextMeasure for Fixed {
    fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
        (
            text.chars().count() as f32 * style.size * 0.55,
            style.size * 1.33,
        )
    }
}

fn window() -> DocWindow {
    let mut engine = Engine::with_bundled_fonts();
    engine.new_document("a.docx").expect("new");
    let mut w = DocWindow::new(DocView::new(engine).expect("view"), "en-US", "a.docx");
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1500.0,
            height: 1102.0,
        },
        &Fixed,
    );
    w
}

/// Records the pictures drawn.
#[derive(Default)]
struct Images(Vec<(u64, [f32; 4])>);
impl Painter for Images {
    fn clear(&mut self, _: Rgba) {}
    fn fill_rect(&mut self, _: f32, _: f32, _: f32, _: f32, _: Rgba) {}
    fn page(&mut self, _: usize, _: i32, _: i32) {}
    fn text(&mut self, _: &str, _: f32, _: f32, _: TextStyle, _: Rgba) {}
    fn path(&mut self, _: &[PathSeg], _: Option<Rgba>, _: Option<(Rgba, f32)>) {}
    fn push_clip(&mut self, _: f32, _: f32, _: f32, _: f32) {}
    fn pop_clip(&mut self) {}
    fn image(&mut self, id: u64, dest: [f32; 4]) {
        self.0.push((id, dest));
    }
}

#[test]
fn new_window_and_side_by_side_ask_the_host() {
    let mut w = window();
    assert!(!w.action("view.newWindow"));
    assert!(w.take_requests().contains(&Request::NewWindow));
    // One window: the side-by-side commands are dimmed on the View tab.
    assert!(w.run_interface_command("tab:view", &Fixed));
    w.handle(Input::Tick, &Fixed);
    w.refresh(&Fixed);
    assert!(!w.ribbon.extra_dimmed("view.newWindow"));
    assert!(w.ribbon.extra_dimmed("view.sideBySide"));
    assert!(w.ribbon.extra_dimmed("view.syncScroll"));
    // Two windows: View Side by Side lights up; Synchronous Scrolling follows it.
    w.set_window_count(2);
    w.handle(Input::Tick, &Fixed);
    w.refresh(&Fixed);
    assert!(!w.ribbon.extra_dimmed("view.sideBySide"));
    assert!(w.ribbon.extra_dimmed("view.syncScroll"));
    assert!(w.action("view.sideBySide"));
    assert!(w.take_requests().contains(&Request::SideBySideLink {
        side: true,
        sync: true
    }));
    assert!(w.r18_on_ids().contains(&"view.syncScroll"));
    w.handle(Input::Tick, &Fixed);
    w.refresh(&Fixed);
    assert!(!w.ribbon.extra_dimmed("view.syncScroll"));
    assert!(!w.ribbon.extra_dimmed("view.resetWindow"));
    assert!(w.action("view.syncScroll"));
    assert!(w.take_requests().contains(&Request::SideBySideLink {
        side: true,
        sync: false
    }));
    assert_eq!(w.side_by_side(), (true, false));
    assert!(!w.action("view.resetWindow"));
    assert!(w.take_requests().contains(&Request::SideBySide));
    // The host ends it (the other window closed).
    w.set_side_by_side(false, false);
    assert_eq!(w.r18_on_ids(), Vec::<&str>::new());
}

#[test]
fn screenshot_gallery_shows_thumbnails_and_screen_clipping() {
    let mut w = window();
    assert!(w.show_screenshot_gallery(vec![
        (101, "Notes".into(), (200, 140)),
        (202, "Browser".into(), (200, 120)),
    ]));
    assert!(w.screenshot_gallery_open());
    let mut p = Images::default();
    w.paint(&mut p);
    let ids: Vec<u64> = p.0.iter().map(|i| i.0).collect();
    assert_eq!(ids, vec![101, 202]);
    // Thumbnails keep their shape.
    let [_, _, tw, th] = p.0[1].1;
    assert!((tw / th - 200.0 / 120.0).abs() < 0.05, "{tw}x{th}");
    // Clicking the second thumbnail captures that window.
    let [x, y, dw, dh] = p.0[1].1;
    assert!(w.handle(
        Input::Press {
            x: x + dw / 2.0,
            y: y + dh / 2.0,
            count: 1,
            shift: false
        },
        &Fixed
    ));
    assert!(!w.screenshot_gallery_open());
    assert!(w.take_requests().contains(&Request::CaptureWindow(202)));
    // Screen Clipping is the row at the bottom.
    w.show_screenshot_gallery(Vec::new());
    let rect = w.r18.shots.as_ref().expect("open").rect;
    w.handle(
        Input::Press {
            x: rect[0] + 40.0,
            y: rect[1] + rect[3] - 32.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    assert!(w.take_requests().contains(&Request::ScreenClip));
    // Esc and a click outside close it.
    w.show_screenshot_gallery(Vec::new());
    w.handle(
        Input::Key(crate::Key::Escape, crate::Mods::default()),
        &Fixed,
    );
    assert!(!w.screenshot_gallery_open());
    w.show_screenshot_gallery(Vec::new());
    w.handle(
        Input::Press {
            x: 5.0,
            y: 1000.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    assert!(!w.screenshot_gallery_open());
    assert_eq!(w.take_requests(), Vec::new());
}

#[test]
fn every_tab_fits_without_scrolling_from_1500_to_600_px() {
    let tabs = [
        "home",
        "insert",
        "design",
        "layout",
        "references",
        "mailings",
        "review",
        "view",
    ];
    // Down to 1000 px every tab fits; narrower, the reference scrolls a tab only once all
    // its groups are drop-down buttons (Insert and Review at 800 px), and so does BlueDoc.
    for width in [1500.0, 1200.0, 1000.0, 800.0, 600.0] {
        let mut w = window();
        w.handle(
            Input::Resize {
                width,
                height: 1102.0,
            },
            &Fixed,
        );
        for tab in tabs {
            assert!(w.run_interface_command(&format!("tab:{tab}"), &Fixed));
            w.handle(Input::Tick, &Fixed);
            w.refresh(&Fixed);
            if !w.ribbon.overflows() {
                continue;
            }
            assert!(width < 1000.0, "{tab} scrolls at {width} px");
            let label = format!("ribbon.tabs.{tab}");
            for g in crate::doc_scale18::GROUPS
                .iter()
                .chain(crate::doc_scale18::GROUPS2.iter())
                .filter(|g| g.1 == label)
            {
                assert!(
                    w.ribbon
                        .shown_rect(crate::ribbon::RibbonHit::Command(g.0))
                        .is_some(),
                    "{tab} scrolls at {width} px with {} not collapsed",
                    g.2
                );
            }
        }
    }
}

#[test]
fn a_collapsed_group_opens_its_panel() {
    let mut w = window();
    w.handle(
        Input::Resize {
            width: 700.0,
            height: 1102.0,
        },
        &Fixed,
    );
    assert!(w.run_interface_command("tab:home", &Fixed));
    w.handle(Input::Tick, &Fixed);
    w.refresh(&Fixed);
    assert!(w
        .ribbon
        .rect_of(crate::ribbon::RibbonHit::Command("r18g.paragraph"))
        .is_some());
    assert!(w.action("r18g.paragraph"));
    assert_eq!(w.r16.pop.as_ref().map(|p| p.id), Some("r18g.paragraph"));
    assert!(w.action("r18g.paragraph"));
    assert!(w.r16.pop.is_none());
}
