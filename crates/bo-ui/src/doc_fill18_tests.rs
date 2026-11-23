//! Layout round 8: Design › Page Color › Fill Effects… opens the Fill Effects dialog; OK sets
//! the page fill effect (one undo step) and the dialog reopens on the fill's tab.
use super::super::DocWindow;
use crate::doc_forms::{DocFormKind, FormOut};
use crate::{DocView, Input, TextMeasure, TextStyle};
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

/// Press OK.
fn enter(w: &mut DocWindow) {
    let f = w.r3.form.as_mut().expect("form");
    f.layout(&Fixed, (1500.0, 1102.0), 1.25);
    let (x, y) = f.point_of("ok").expect("OK");
    let out = f.input(&Input::Press {
        x,
        y,
        count: 1,
        shift: false,
    });
    assert!(matches!(out, FormOut::Run(..)), "{out:?}");
    w.form_out(out);
}

#[test]
fn fill_effects_sets_a_gradient_then_a_texture_and_a_picture() {
    let mut w = window();
    assert!(w.action("design.pageColor.fill"));
    let f = w.r3.form.as_mut().expect("Fill Effects");
    assert_eq!(f.kind, DocFormKind::FillEffects);
    f.layout(&Fixed, (1500.0, 1102.0), 1.25);
    enter(&mut w);
    assert!(w.r3.form.is_none());
    let fill = w
        .view
        .engine()
        .document()
        .and_then(|d| d.design.background_fill.clone())
        .expect("fill");
    assert_eq!(fill.kind, bo_model::PageFillKind::Gradient);
    // Texture tab: a BlueOffice texture tiled over the page; reopened on that tab.
    assert!(w.action("design.pageColor.fill"));
    w.r3.form.as_mut().expect("form").set_tab(1);
    enter(&mut w);
    let d = w.view.engine().document().expect("doc");
    let fill = d.design.background_fill.clone().expect("fill");
    assert_eq!(fill.kind, bo_model::PageFillKind::Tile);
    assert!(d.media.contains_key(fill.image.as_deref().expect("image")));
    assert!(w.action("design.pageColor.fill"));
    assert_eq!(w.r3.form.as_ref().expect("form").form_tab_for_test(), 1);
    // Picture tab: Select Picture… asks the host; the picked file goes to the dialog.
    let f = w.r3.form.as_mut().expect("form");
    f.set_tab(3);
    let out = f.click_for_test("selectPicture");
    w.form_out(out);
    assert!(w.take_requests().contains(&crate::Request::PickPicture));
    let png = crate::slide_textures22::png(0).expect("png");
    assert!(w.insert_picture(png, "photo.png"));
    enter(&mut w);
    let fill = w
        .view
        .engine()
        .document()
        .and_then(|d| d.design.background_fill.clone())
        .expect("fill");
    assert_eq!(fill.kind, bo_model::PageFillKind::Frame);
    assert_eq!(fill.title.as_deref(), Some("photo.png"));
}
