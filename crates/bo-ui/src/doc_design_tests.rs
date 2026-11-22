//! BlueDoc Design tab: Themes / Colors / Fonts / Paragraph Spacing drop-downs.
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

fn window() -> DocWindow {
    let mut engine = Engine::with_bundled_fonts();
    engine.new_document("a.docx").unwrap();
    let mut w = DocWindow::new(DocView::new(engine).unwrap(), "en-US", "a.docx");
    w.handle(
        Input::Resize {
            width: 1800.0,
            height: 900.0,
        },
        &Fixed,
    );
    w
}

fn press(w: &mut DocWindow, (x, y): (f32, f32)) {
    w.handle(Input::Move { x, y }, &Fixed);
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

#[test]
fn design_tab_has_the_document_formatting_drop_downs() {
    let mut w = window();
    w.run_interface_command("tab:design", &Fixed);
    w.handle(Input::Tick, &Fixed);
    for id in [
        "design.themes",
        "design.colors",
        "design.fonts",
        "design.paragraphSpacing",
    ] {
        assert!(
            w.ribbon.rect_of(RibbonHit::Command(id)).is_some(),
            "{id} on Design"
        );
    }
    w.action("design.themes");
    let at = w
        .design_gallery
        .as_ref()
        .map(|g| g.center(1))
        .expect("themes open");
    press(&mut w, at);
    assert!(w.design_gallery.is_none());
    assert_eq!(w.design_theme.as_deref(), Some("bamboo"));
    assert!(w.view.format().can_undo, "theme applied as one step");
    w.action("design.paragraphSpacing");
    let at = w
        .design_gallery
        .as_ref()
        .map(|g| g.center(3))
        .expect("open");
    press(&mut w, at);
    assert!(w.design_gallery.is_none());
    w.action("design.colors");
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(w.design_gallery.is_none());
}

#[test]
fn style_set_gallery_and_page_borders() {
    let mut w = window();
    w.run_interface_command("tab:design", &Fixed);
    w.handle(Input::Tick, &Fixed);
    // Seven style set tiles and the ▾ column sit between Themes and Colors.
    let x_of = |w: &DocWindow, id: &'static str| {
        w.ribbon
            .rect_of(RibbonHit::Command(id))
            .map_or_else(|| panic!("{id} on Design"), |r| r[0])
    };
    let themes = x_of(&w, "design.themes");
    let first = x_of(&w, crate::doc_design::STYLE_TILES[0].0);
    let last = x_of(&w, crate::doc_design::STYLE_TILES[6].0);
    let colors = x_of(&w, "design.colors");
    assert!(themes < first && first < last && last < colors);
    for id in ["design.effects", "design.setDefault", "design.pageBorders"] {
        assert!(w.ribbon.rect_of(RibbonHit::Command(id)).is_some(), "{id}");
    }
    // Effects and Set as Default work since round 3 (doc_r3).
    assert!(commands::enabled("design.effects", w.view.format()));
    assert!(commands::enabled("design.setDefault", w.view.format()));
    // A tile applies its style set in one step.
    let r = w
        .ribbon
        .rect_of(RibbonHit::Command("design.styleSet.capital"))
        .expect("tile");
    press(&mut w, (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0));
    assert_eq!(w.design_style_set.as_deref(), Some("capital"));
    assert!(w.view.format().can_undo);
    // The ▾ lists the sets.
    let more = crate::doc_design::more_rect(&w.ribbon, 1.0).expect("▾ column");
    press(&mut w, (more[0] + more[2] / 2.0, more[1] + more[3] / 2.0));
    assert!(w.design_gallery.is_some(), "▾ lists the sets");
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    // Page Borders: Borders and Shading on its Page Border tab; Box, Double, OK puts borders
    // on the pages (round 12).
    w.action("design.pageBorders");
    w.handle(Input::Tick, &Fixed);
    let f = w.r3.form.as_mut().expect("Borders and Shading");
    assert_eq!(f.kind, crate::doc_forms::DocFormKind::BordersShading);
    assert!(f.canvas_for_test("pSet1", 10.0, 10.0));
    // Scroll the Style list two rows, then the fifth row shown is Double.
    f.canvas_for_test("pStyle", 195.0, 160.0);
    f.canvas_for_test("pStyle", 195.0, 160.0);
    f.canvas_for_test("pStyle", 30.0, 2.0 + 4.0 * 33.0 + 10.0);
    assert_eq!(
        f.bs_edge_for_test(1, 0).map(|e| e.0).as_deref(),
        Some("double")
    );
    let out = f.input(&Input::Key(Key::Enter, Mods::default()));
    w.form_out(out);
    assert!(w.r3.form.is_none());
    let doc = w.view.engine().document().expect("document");
    let pb = doc.sections[0].page_borders.as_ref().expect("page borders");
    assert!(format!("{pb:?}").contains("ouble"));
}
