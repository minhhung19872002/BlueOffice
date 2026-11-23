//! BlueDoc parity round 19: Picture Format › Picture Styles.
use super::super::DocWindow;
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

fn window_with_picture() -> DocWindow {
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
    let png = bo_raster::rgba_to_png(bo_raster::codecs::Rgba {
        width: 16,
        height: 16,
        pixels: vec![180; 16 * 16 * 4],
    })
    .expect("png");
    assert!(w.view.insert_picture(png, "p.png"));
    w.view.exec(bo_engine::edit::Command::Move {
        motion: bo_engine::nav::Motion::Left,
        extend: true,
    });
    w.handle(Input::Tick, &Fixed);
    w
}

fn format(w: &DocWindow) -> bo_model::picture::PictureFormat {
    w.r19_current()
}

#[test]
fn picture_styles_are_no_longer_greyed() {
    let w = window_with_picture();
    for id in super::IDS {
        assert!(!crate::doc_tabs7::GREYED.contains(&id), "{id}");
    }
    drop(w);
}

#[test]
fn border_effects_artistic_and_quick_styles_run_from_the_menus() {
    let mut w = window_with_picture();
    // Picture Border ▾: No Outline, 12 colours, 9 weights, 6 dashes.
    assert!(w.action("picture.border"));
    assert_eq!(w.menu.as_ref().map(|m| m.items().len()), Some(28));
    assert!(w.action("r19.bc:0070C0"));
    assert!(w.action("r19.bw:3"));
    assert!(w.action("r19.bd:dash"));
    let l = format(&w).line.expect("border");
    assert_eq!(
        (l.color, l.width, l.dash.as_deref()),
        (0x70C0, 38_100, Some("dash"))
    );
    // Picture Effects ▾ lists the categories; each opens its presets.
    assert!(w.action("picture.effects"));
    assert_eq!(w.menu.as_ref().map(|m| m.items().len()), Some(7));
    assert!(w.action("r19.fxmenu:shadow"));
    assert_eq!(w.menu.as_ref().map(|m| m.items().len()), Some(19));
    assert!(w.action("r19.fx:shadow=outerBottomRight"));
    assert!(w.action("r19.fx:glow=8:ED7D31"));
    assert!(w.action("r19.fx:softEdge=5"));
    assert!(w.action("r19.fx:rotation=perspectiveLeft"));
    let fx = format(&w).effects.expect("effects");
    assert!(fx.shadow.is_some() && fx.glow.is_some() && fx.soft_edge.is_some());
    assert_eq!(fx.camera.as_deref(), Some("perspectiveLeft"));
    // The checked entry follows the picture.
    assert!(w.action("r19.fxmenu:rotation"));
    let checked: Vec<String> = w
        .menu
        .as_ref()
        .map(|m| {
            m.items()
                .iter()
                .filter(|i| i.checked)
                .map(|i| i.label.clone())
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(checked, vec!["Perspective Left".to_owned()]);
    // Artistic Effects ▾: None + 22 effects.
    assert!(w.action("picture.artisticEffects"));
    assert_eq!(w.menu.as_ref().map(|m| m.items().len()), Some(23));
    assert!(w.action("r19.art:artisticPencilGrayscale"));
    assert_eq!(
        format(&w).artistic.map(|a| a.name),
        Some("artisticPencilGrayscale".to_owned())
    );
    // Quick Styles replace the border and effects; Preset "none" clears the effects.
    assert!(w.action("picture.quickStyles"));
    assert!(w.action("r19.quick:1"));
    let f = format(&w);
    assert_eq!(f.line.as_ref().map(|l| l.color), Some(0));
    assert!(w.action("r19.fx:preset=none"));
    assert!(w.action("r19.bc:none"));
    let f = format(&w);
    assert!(f.effects.is_none() && f.line.is_none());
}

#[test]
fn each_window_puts_its_own_view_back_on_the_shared_engine() {
    use bo_engine::views::ViewMode;
    let mut w = window_with_picture();
    assert!(w.action("view.layout.web"));
    assert_eq!(w.view.engine().view_layout().mode, ViewMode::Web);
    // Another window (print layout) used the engine meanwhile: this one is still Web.
    assert!(!w.restore_view_mode(), "same view: nothing to do");
    w.set_view_mode_name("print");
    assert!(w.restore_view_mode());
    assert_eq!(w.view.engine().view_layout().mode, ViewMode::Print);
    assert_eq!(w.view_mode_name(), "print");
}
