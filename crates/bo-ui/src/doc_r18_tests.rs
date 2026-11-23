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
            // Tab labels shrink (clipped) instead of the tab strip scrolling.
            assert!(!w.ribbon.tabs_scroll(), "tab strip scrolls at {width} px");
            // A gallery never shows as a sliver: Styles is its drop-down button then.
            if tab == "home"
                && w.ribbon
                    .shown_rect(crate::ribbon::RibbonHit::StylesMore)
                    .is_some()
            {
                assert!(
                    w.ribbon
                        .shown_rect(crate::ribbon::RibbonHit::Style(0))
                        .is_some(),
                    "Styles gallery without a whole tile at {width} px"
                );
            }
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

fn tiny_png(_: &str) -> Option<(Vec<u8>, String)> {
    let png = bo_raster::rgba_to_png(bo_raster::codecs::Rgba {
        width: 2,
        height: 2,
        pixels: vec![200; 16],
    })?;
    Some((png, "p.png".into()))
}

fn nothing(_: &str) -> Option<(Vec<u8>, String)> {
    None
}

#[test]
fn online_pictures_download_in_the_background_with_cancel() {
    let mut w = window();
    let before = w.view.engine().document().map_or(0, |d| d.media.len());
    assert!(w.r18_start_download("https://example.test/p.png".into(), tiny_png));
    assert!(w.downloading());
    // Typing waits while the panel shows.
    assert!(!w.handle(Input::Text("x".into()), &Fixed));
    for _ in 0..200 {
        w.handle(Input::Tick, &Fixed);
        if !w.downloading() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(!w.downloading(), "the picture arrived");
    let after = w.view.engine().document().map_or(0, |d| d.media.len());
    assert_eq!(after, before + 1, "inserted");
    // A failed download says so until closed; Cancel / Esc stop waiting.
    assert!(w.r18_start_download("https://example.test/x".into(), nothing));
    for _ in 0..200 {
        w.handle(Input::Tick, &Fixed);
        if w.r18.download.as_ref().is_some_and(|d| d.failed) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(w.downloading());
    w.handle(
        Input::Key(crate::Key::Escape, crate::Mods::default()),
        &Fixed,
    );
    assert!(!w.downloading());
}

#[test]
fn shape_format_fill_outline_styles_and_change_shape() {
    let mut w = window();
    w.run_interface_command("text:Shape here", &Fixed);
    w.r3_run(vec![serde_json::json!({
        "cmd": "insertShape", "preset": "rect", "width": 120.0, "height": 60.0
    })]);
    let shape = |w: &DocWindow| -> Option<bo_model::Shape> {
        fn find(blocks: &[bo_model::Block]) -> Option<bo_model::Shape> {
            blocks.iter().find_map(|b| match b {
                bo_model::Block::Paragraph(p) => p.runs.iter().find_map(|r| {
                    r.content.iter().find_map(|c| match c {
                        bo_model::RunContent::Image(img) => img.shape.as_deref().cloned(),
                        _ => None,
                    })
                }),
                _ => None,
            })
        }
        w.view
            .engine()
            .document()
            .and_then(|d| d.sections.iter().find_map(|s| find(&s.blocks)))
    };
    assert!(shape(&w).is_some(), "a shape was inserted");
    // The shape is selected (as a click on it selects it).
    w.view.exec(bo_engine::edit::Command::Move {
        motion: bo_engine::nav::Motion::Left,
        extend: true,
    });
    w.handle(Input::Tick, &Fixed);
    // Shape Fill ▾ lists No Fill and the colours; a colour fills the shape.
    assert!(w.action("shape.fill"));
    assert_eq!(w.menu.as_ref().map(|m| m.items().len()), Some(11));
    assert!(w.action("r18.fill:FF0000"));
    let s = shape(&w).expect("shape");
    assert_eq!(s.fill.map(|c| (c.r, c.g, c.b)), Some((255, 0, 0)));
    // Shape Outline: weight and dash; No Outline removes it.
    assert!(w.action("r18.olw:3"));
    assert!(w.action("r18.old:dash"));
    let l = shape(&w).and_then(|s| s.line).expect("line");
    assert_eq!((l.width.0, l.dash.as_deref()), (38_100, Some("dash")));
    assert!(w.action("r18.olc:none"));
    assert!(shape(&w).expect("shape").line.is_none());
    // A Shape Styles entry sets fill and outline; Change Shape keeps the formatting.
    assert!(w.action("shape.style.1"));
    let s = shape(&w).expect("shape");
    assert_eq!(s.fill.map(|c| (c.r, c.g, c.b)), Some((0xED, 0x7D, 0x31)));
    assert!(s.line.is_some());
    assert!(w.action("r18.preset:ellipse"));
    assert_eq!(shape(&w).expect("shape").preset, "ellipse");
    // No Fill.
    assert!(w.action("r18.fill:none"));
    assert!(shape(&w).expect("shape").fill.is_none());
}
