//! BlueDoc Design tab: Themes / Colors / Fonts / Paragraph Spacing drop-downs, and the
//! tabbed Find and Replace dialog.
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
fn find_replace_and_go_to_are_tabs_of_one_dialog() {
    let mut w = window();
    w.action("edit.replace");
    let d = w.dialog.as_ref().expect("dialog");
    assert_eq!(d.kind, DialogKind::Replace);
    assert_eq!(d.page(), 1);
    w.handle(Input::Text("needle".into()), &Fixed);
    // Ctrl+PageUp shows the Find tab, keeping the text.
    w.handle(
        Input::Key(
            Key::PageUp,
            Mods {
                ctrl: true,
                ..Mods::default()
            },
        ),
        &Fixed,
    );
    let d = w.dialog.as_ref().expect("still open");
    assert_eq!(d.kind, DialogKind::Find);
    assert_eq!(d.page(), 0);
    assert!(d.values()[0].contains("needle"));
    w.handle(
        Input::Key(
            Key::PageUp,
            Mods {
                ctrl: true,
                ..Mods::default()
            },
        ),
        &Fixed,
    );
    let d = w.dialog.as_ref().expect("still open");
    assert_eq!(d.kind, DialogKind::GoTo);
    assert_eq!(d.page(), 2);
}
