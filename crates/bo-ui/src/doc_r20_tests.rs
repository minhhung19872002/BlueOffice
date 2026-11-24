//! BlueDoc parity round 20: Shape Format fills, Shape Effects and WordArt Styles.
use super::super::DocWindow;
use crate::{DocView, Input, TextMeasure, TextStyle};
use bo_engine::Engine;
use serde_json::json;

struct Fixed;
impl TextMeasure for Fixed {
    fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
        (
            text.chars().count() as f32 * style.size * 0.55,
            style.size * 1.33,
        )
    }
}

/// A window with a text box holding "Art", selected.
fn window_with_shape() -> DocWindow {
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
    w.run_interface_command("text:Shape here", &Fixed);
    w.r3_run(vec![json!({
        "cmd": "insertShape", "preset": "rect", "width": 120.0, "height": 60.0
    })]);
    w.view.exec(bo_engine::edit::Command::Move {
        motion: bo_engine::nav::Motion::Left,
        extend: true,
    });
    w.handle(Input::Tick, &Fixed);
    w
}

fn shape(w: &DocWindow) -> bo_model::Shape {
    w.r20_shape()
        .and_then(|i| i.shape.map(|s| *s))
        .expect("selected shape")
}

fn style(w: &DocWindow) -> bo_model::shape_style::ShapeStyle {
    shape(w).style.map(|s| *s).unwrap_or_default()
}

fn menu_len(w: &DocWindow) -> Option<usize> {
    w.menu.as_ref().map(|m| m.items().len())
}

#[test]
fn shape_effects_and_wordart_are_no_longer_greyed() {
    for id in super::IDS {
        assert!(!crate::doc_tabs7::GREYED.contains(&id), "{id}");
    }
}

#[test]
fn shape_fill_gradient_texture_and_picture() {
    let mut w = window_with_shape();
    assert!(w.action("shape.fill"));
    assert_eq!(menu_len(&w), Some(17));
    // Gradient ▸: No Gradient, nine light and nine dark variations, More Gradients….
    assert!(w.action("r20.menu:shape.fill/gradient"));
    assert_eq!(menu_len(&w), Some(20));
    assert!(w.action("r20.grad:dark:4"));
    let g = style(&w).gradient.expect("gradient");
    assert_eq!(g.kind, "circle");
    // Texture ▸: the textures and More Textures….
    assert!(w.action("r20.menu:shape.fill/texture"));
    assert_eq!(menu_len(&w), Some(crate::slide_textures22::NAMES.len() + 1));
    assert!(w.action("r20.tex:2"));
    let st = style(&w);
    assert!(st.gradient.is_none());
    assert!(st.picture.as_ref().is_some_and(|p| p.tile));
    // Picture…: the host's picture fills the shape, stretched.
    assert!(!w.action("r20.picture"));
    let png = bo_raster::rgba_to_png(bo_raster::codecs::Rgba {
        width: 4,
        height: 4,
        pixels: vec![200; 64],
    })
    .expect("png");
    assert!(w.insert_picture(png, "fill.png"));
    assert!(style(&w).picture.as_ref().is_some_and(|p| !p.tile));
    // A plain colour replaces the picture fill.
    assert!(w.action("r18.fill:00B050"));
    assert!(shape(&w).style.is_none());
}

#[test]
fn fill_effects_for_a_shape_fills_the_shape() {
    let mut w = window_with_shape();
    // Round 21: More Gradients… opens the Format Shape pane; the Fill Effects dialog itself.
    assert!(w.r20_fill_effects(0));
    assert!(w.r20.shape_dialog);
    // A page gradient from the dialog becomes the shape's gradient.
    let cmds = w.r20_translate(vec![json!({
        "cmd": "setPageFill", "color": "FF0000",
        "fill": { "kind": "gradient", "angle": -90, "color2": { "r": 0, "g": 0, "b": 255 } }
    })]);
    assert_eq!(cmds[0]["cmd"], "shapeGradient");
    assert_eq!(cmds[0]["angle"], json!(0.0));
    w.r3.form = None;
    w.r3_run(cmds);
    let g = style(&w).gradient.expect("gradient");
    assert_eq!(g.stops.len(), 2);
    assert_eq!(g.stops[0].color, 0xFF_0000);
    // A pattern keeps its colours as a tiled picture.
    let bmp = crate::doc_forms::r18::pattern_bmp([0xAA; 8]);
    let p = super::shape_fill_of(&json!({
        "cmd": "setPageFill", "color": "FF0000",
        "fill": { "kind": "pattern", "color2": { "r": 0, "g": 0, "b": 255 } },
        "picture": super::b64(&bmp)
    }))
    .expect("pattern");
    assert_eq!(p["tile"], json!(true));
    // Page fills stay page fills when the dialog was not opened for a shape.
    assert!(!w.r20.shape_dialog);
    let page = w.r20_translate(vec![json!({ "cmd": "setPageFill", "color": "FFFFFF" })]);
    assert_eq!(page[0]["cmd"], "setPageFill");
}

#[test]
fn shape_effects_and_outline_colours() {
    let mut w = window_with_shape();
    assert!(w.action("shape.effects"));
    assert_eq!(menu_len(&w), Some(7));
    assert!(w.action("r20.menu:shape.effects/shadow"));
    assert!(w.action("r20.fx:shadow=outerBottomRight"));
    assert!(w.action("r20.fx:glow=8:ED7D31"));
    assert!(w.action("r20.fx:bevel=circle"));
    assert!(w.action("r20.fx:rotation=isometricLeftDown"));
    let fx = style(&w).effects.expect("effects");
    assert!(fx.shadow.is_some() && fx.glow.is_some() && fx.bevel.is_some());
    assert_eq!(fx.camera.as_deref(), Some("isometricLeftDown"));
    assert!(w.action("r20.fx:preset=none"));
    // Text › Align Text.
    assert!(w.action("shape.alignText"));
    assert_eq!(menu_len(&w), Some(3));
    assert!(w.action("r20.anchor:b"));
    assert_eq!(shape(&w).anchor, bo_model::TextAnchor::Bottom);
    assert!(style(&w).effects.is_none());
    // More Outline Colors… answers through the Colors dialog.
    assert!(w.action("shape.outline"));
    assert!(w.action("r20.moreLine"));
    assert!(w.action("r16.fontColor:123456"));
    let l = shape(&w).line.expect("line");
    assert_eq!((l.color.r, l.color.g, l.color.b), (0x12, 0x34, 0x56));
}

#[test]
fn wordart_styles_change_the_shape_text() {
    let mut w = window_with_shape();
    // The shape needs text: a text box.
    w.r3_run(vec![json!({
        "cmd": "insertShape", "preset": "rect", "width": 120.0, "height": 60.0, "textBox": true
    })]);
    w.view.exec(bo_engine::edit::Command::Move {
        motion: bo_engine::nav::Motion::Left,
        extend: true,
    });
    w.handle(Input::Tick, &Fixed);
    assert!(shape(&w).text.is_some(), "a text box");
    // Round 22: Quick Styles is a gallery of thumbnails.
    assert!(w.action("shape.wordArt"));
    assert!(w.r22.gallery.as_ref().is_some_and(|g| g.cells().len() > 2));
    for id in ["shape.textFill", "shape.textOutline", "shape.textEffects"] {
        assert!(w.action(id), "{id}");
        assert!(menu_len(&w).is_some_and(|n| n > 2), "{id}");
    }
    assert!(w.action("r20.wa:3"));
    assert!(w.action("r20.tf:C00000"));
    assert!(w.action("r20.to:0070C0"));
    assert!(w.action("r20.te:glow=1"));
    let text = shape(&w).text.expect("text");
    let bo_model::Block::Paragraph(p) = &text[0] else {
        panic!("paragraph");
    };
    let props = p
        .runs
        .first()
        .map(|r| r.props.clone())
        .or_else(|| p.props.mark_run.as_deref().cloned())
        .expect("run properties");
    assert_eq!(props.color.map(|c| (c.r, c.g, c.b)), Some((0xC0, 0, 0)));
    let fx = props.effects.as_deref().expect("effects");
    assert!(fx.glow.is_some() && fx.outline.is_some());
}
