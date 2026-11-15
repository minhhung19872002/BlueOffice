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

/// A window on the photo album sample with its first picture selected.
fn picture_window() -> SlideWindow {
    let mut engine = Engine::with_bundled_fonts();
    let name = "slide-album-photos.pptx";
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
    let found = (0..w.view().slide_count()).find_map(|i| {
        let s = &w.view().engine().presentation()?.slides[i];
        s.data.shapes.iter().find_map(|n| match n {
            bo_model::slide::ShapeNode::Picture(p) => Some((i, p.nv.id)),
            _ => None,
        })
    });
    let (slide, id) = found.expect("a picture in the album");
    w.view.go_to(slide);
    w.activate_context = true;
    w.view.run(SlideCommand::Select { ids: vec![id] });
    w.handle(Input::Tick, &Fixed);
    w
}

#[test]
fn picture_galleries_apply_adjustments() {
    let mut w = picture_window();
    assert_eq!(w.context, ins::Context::Picture);
    click_command(&mut w, "sl.corrections");
    let g = w.gallery.as_ref().expect("corrections gallery");
    assert_eq!(g.cells.len(), 30);
    assert!(g.cells.iter().all(|(r, _)| r[2] > 0.0));
    // Brightness +40 %, contrast +40 % (first row of the grid, last column).
    let r = g.cells[9].0;
    click(&mut w, r[0] + 5.0, r[1] + 5.0);
    assert!(w.gallery.is_none());
    let info = w.picture_info().expect("picture");
    assert_eq!((info.fx.bright, info.fx.contrast), (40_000, 40_000));
    click_command(&mut w, "sl.transparency");
    let r = w.gallery.as_ref().expect("transparency").cells[3].0;
    click(&mut w, r[0] + 5.0, r[1] + 5.0);
    let info = w.picture_info().expect("picture");
    assert_eq!(info.fx.alpha, Some(50_000));
    assert_eq!(info.fx.bright, 40_000, "transparency keeps the corrections");
    click_command(&mut w, "sl.picColor");
    let r = w.gallery.as_ref().expect("color").cells[8].0;
    click(&mut w, r[0] + 5.0, r[1] + 5.0);
    assert!(w.picture_info().expect("picture").fx.grayscale);
    click_command(&mut w, "sl.resetPicture");
    assert!(w.picture_info().expect("picture").fx.is_identity());
}

#[test]
fn remove_background_marks_and_keeps_changes() {
    let mut w = picture_window();
    assert!(w.picture_info().expect("picture").removable);
    click_command(&mut w, "sl.removeBg");
    assert!(w.bg_mode.is_some());
    assert!(
        w.command_rect("sl.bgKeep").is_some(),
        "Background Removal tab"
    );
    assert!(w.bg_mode.as_ref().and_then(|m| m.preview.clone()).is_some());
    click_command(&mut w, "sl.bgRemove");
    // A stroke across the picture's corner marks an area to remove.
    let f = w.bg_mode.as_ref().expect("mode").frame;
    let (ox, oy) = w.view().slide_origin();
    let k = w.view().scale();
    let at = |u: f64, v: f64| {
        (
            ox + (f[0] + u * f[2]) as f32 * k,
            oy + (f[1] + v * f[3]) as f32 * k,
        )
    };
    let (x0, y0) = at(0.05, 0.05);
    let (x1, y1) = at(0.2, 0.1);
    press(&mut w, x0, y0, 1);
    w.handle(Input::Drag { x: x1, y: y1 }, &Fixed);
    w.handle(Input::Release { x: x1, y: y1 }, &Fixed);
    let m = w.bg_mode.as_ref().expect("mode");
    assert_eq!(m.spec.marks.len(), 1);
    assert!(!m.spec.marks[0].keep);
    click_command(&mut w, "sl.bgDone");
    assert!(w.bg_mode.is_none());
    assert!(w.picture_info().expect("picture").removal.is_some());
    assert!(
        w.command_rect("sl.removeBg").is_some(),
        "Picture Format is back"
    );
}

#[test]
fn compress_dialog_reports() {
    let mut w = picture_window();
    w.run_action("sl.compress");
    assert!(matches!(
        w.dialog.as_ref().map(|(_, k)| *k),
        Some(ins::SlideDialog::Compress)
    ));
    w.compress_button(
        crate::dialog::DialogButton::Ok,
        &["96".into(), "1".into(), "1".into()],
    );
    assert!(w.dialog.is_some(), "the result stays shown");
}

#[test]
fn draw_borders_pen_and_borders_menu() {
    let mut w = table_window();
    for id in [
        "sl.tblBorders",
        "sl.tblEffects",
        "sl.penStyle",
        "sl.penWeight",
        "sl.penColor",
        "sl.textQuick",
    ] {
        assert!(w.command_rect(id).is_some(), "{id}");
    }
    // 3 pt pen (the seventh weight), then All Borders.
    click_command(&mut w, "sl.penWeight");
    for _ in 0..7 {
        w.handle(Input::Key(Key::Down, Mods::default()), &Fixed);
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!((w.pen.width - 3.0).abs() < 1e-9, "{}", w.pen.width);
    click_command(&mut w, "sl.tblBorders");
    w.handle(Input::Key(Key::Down, Mods::default()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let id = w.view().selection()[0].id;
    let p = w.view().engine().presentation().expect("deck");
    let t = p.slides[w.view().current()]
        .data
        .shapes
        .iter()
        .find_map(|n| match n {
            bo_model::slide::ShapeNode::Frame(f) if f.nv.id == id => match &f.content {
                bo_model::slide::GraphicContent::Table(t) => Some(t.clone()),
                bo_model::slide::GraphicContent::Other(_) => None,
            },
            _ => None,
        })
        .expect("table");
    let ln = t.rows[1].cells[1]
        .props
        .as_ref()
        .and_then(|p| p.ln_t.clone())
        .expect("inner top border");
    assert_eq!(ln.w, Some(38_100));
}

fn view_tab(w: &mut SlideWindow) {
    let i = w.base_tabs - 1;
    let [x, y, tw, th] = w.ribbon.rect_of(RibbonHit::Tab(i)).expect("view tab");
    click(w, x + tw / 2.0, y + th / 2.0);
}

#[test]
fn grayscale_ruler_and_page_masters() {
    let mut w = window();
    view_tab(&mut w);
    click_command(&mut w, "view.grayscale");
    assert_eq!(
        w.view().color_mode(),
        crate::slide_view::colormode::ColorMode::Grayscale
    );
    click_command(&mut w, "view.blackWhite");
    click_command(&mut w, "view.color");
    assert_eq!(
        w.view().color_mode(),
        crate::slide_view::colormode::ColorMode::Color
    );
    click_command(&mut w, "view.ruler");
    assert!(w.ruler);
    let mut p = Count::default();
    w.paint(&mut p);
    // Handout Master: its tab first, the view has one page; Close brings the deck back.
    let n = w.view().slide_count();
    click_command(&mut w, "view.handoutMaster");
    assert!(w.page_master.is_some());
    assert_eq!(w.ribbon.active_tab(), 0);
    assert!(w.command_rect("pm.perPage").is_some());
    click_command(&mut w, "pm.header");
    assert!(!w.pm_flags[0]);
    click_command(&mut w, "view.closePageMaster");
    assert!(w.page_master.is_none());
    assert_eq!(w.view().slide_count(), n);
    click_command(&mut w, "view.notesMaster");
    assert!(w.command_rect("pm.slideImage").is_some());
    click_command(&mut w, "view.closePageMaster");
    assert_eq!(w.view().slide_count(), n);
}

#[test]
fn home_slides_group_and_sections() {
    let mut w = window();
    for id in ["slide.new", "slide.layout", "slide.reset", "slide.section"] {
        assert!(w.command_rect(id).is_some(), "{id}");
    }
    assert!(w.view().slide_count() >= 2, "the sample has several slides");
    w.view.go_to(1);
    click_command(&mut w, "slide.section");
    w.handle(Input::Key(Key::Down, Mods::default()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.dialog.is_some(), "section name dialog");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let s = w.view().sections().to_vec();
    assert_eq!(s.len(), 2, "{s:?}");
    assert_eq!(s[1].first, 1);
    // Collapse the second section from its header: its slides leave the pane.
    w.handle(Input::Tick, &Fixed);
    let (_, r) = w.view().section_rows()[1];
    click(&mut w, r[0] + 20.0, r[1] + r[3] / 2.0);
    assert!(w.view().section_collapsed(1));
    // Reset applies the slide's own layout again (one undo step).
    click_command(&mut w, "slide.reset");
    let mut p = Count::default();
    w.paint(&mut p);
}

#[test]
fn draw_table_eraser_and_window_group() {
    let mut w = table_window();
    click_command(&mut w, "sl.drawTable");
    assert!(matches!(w.table_tool, tabletools::TableTool::Draw(_)));
    let c = w.view().canvas_rect();
    let (x0, y0) = (c[0] + 40.0, c[1] + 40.0);
    let n = w.selection_rows().len();
    press(&mut w, x0, y0, 1);
    w.handle(
        Input::Drag {
            x: x0 + 200.0,
            y: y0 + 80.0,
        },
        &Fixed,
    );
    w.handle(
        Input::Release {
            x: x0 + 200.0,
            y: y0 + 80.0,
        },
        &Fixed,
    );
    assert_eq!(w.selection_rows().len(), n + 1, "a drawn table");
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert_eq!(w.table_tool, tabletools::TableTool::Off);
    // View > Window asks the host.
    view_tab(&mut w);
    assert!(w.toggled().contains(&"view.normal"));
    click_command(&mut w, "win.new");
    assert_eq!(w.take_window_request(), Some(WindowRequest::NewWindow));
    click_command(&mut w, "win.switch");
    assert_eq!(w.take_window_request(), Some(WindowRequest::List));
    w.show_windows(&["a — BlueSlide".into(), "b — BlueSlide".into()], 0);
    assert!(w.menu.is_some());
    w.handle(Input::Key(Key::Down, Mods::default()), &Fixed);
    w.handle(Input::Key(Key::Down, Mods::default()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(matches!(
        w.take_window_request(),
        Some(WindowRequest::Activate(_))
    ));
}
