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
    let more = x_of(&w, crate::doc_design::MORE);
    let colors = x_of(&w, "design.colors");
    assert!(themes < first && first < more && more < colors);
    for id in ["design.effects", "design.setDefault", "design.pageBorders"] {
        assert!(w.ribbon.rect_of(RibbonHit::Command(id)).is_some(), "{id}");
    }
    assert!(!commands::enabled("design.effects", w.view.format()));
    // A tile applies its style set in one step.
    let r = w
        .ribbon
        .rect_of(RibbonHit::Command("design.styleSet.capital"))
        .expect("tile");
    press(&mut w, (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0));
    assert_eq!(w.design_style_set.as_deref(), Some("capital"));
    assert!(w.view.format().can_undo);
    // The ▾ lists the sets.
    w.action("design.styleSetMore");
    assert!(w.design_gallery.is_some());
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    // Page Borders: Box, Double, OK puts borders on the pages.
    w.action("design.pageBorders");
    w.handle(Input::Tick, &Fixed);
    let (settings, double, ok) = w
        .borders_dialog
        .as_ref()
        .map(crate::doc_borders::BordersDialog::centers)
        .expect("dialog");
    press(&mut w, settings[1]);
    press(&mut w, double);
    press(&mut w, ok);
    assert!(w.borders_dialog.is_none());
    let doc = w.view.engine().document().expect("document");
    let pb = doc.sections[0].page_borders.as_ref().expect("page borders");
    assert!(format!("{pb:?}").contains("ouble"));
}
