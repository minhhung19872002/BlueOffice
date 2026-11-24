//! BlueDoc parity round 22: thumbnail galleries, Text Direction, Transform.
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

fn style(w: &DocWindow) -> bo_model::shape_style::ShapeStyle {
    w.r22_shape_style().unwrap_or_default()
}

/// Pick the gallery cell whose command is `cmd` with a real press.
fn pick(w: &mut DocWindow, cmd: &str) {
    let g = w.r22.gallery.as_ref().expect("gallery open");
    let (si, ci) = {
        let cells = g.cells();
        let i = cells
            .iter()
            .position(|c| c.2 == cmd)
            .unwrap_or_else(|| panic!("no cell {cmd}"));
        let s = cells[i].0;
        let first = cells.iter().position(|c| c.0 == s).unwrap_or(0);
        (s, i - first)
    };
    let (x, y) = g.center(si, ci);
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
}

#[test]
fn text_direction_menu_turns_the_text() {
    let mut w = window_with_shape();
    assert!(w.action("shape.textDirection"));
    assert_eq!(w.menu.as_ref().map(|m| m.items().len()), Some(3));
    assert!(w.action("r22.vert:vert270"));
    assert_eq!(style(&w).vert.as_deref(), Some("vert270"));
    assert!(w.action("r22.vert:horz"));
    assert!(style(&w).vert.is_none());
}

#[test]
fn transform_gallery_warps_the_text() {
    let mut w = window_with_shape();
    assert!(w.action("r22.transform"));
    let g = w.r22.gallery.as_ref().expect("gallery");
    // No Transform, Follow Path (4) and the Warp presets.
    let cells = g.cells();
    assert_eq!(cells.iter().filter(|c| c.0 == 1).count(), 4);
    assert!(cells.len() > 30, "{}", cells.len());
    pick(&mut w, "r22.warp:textWave1");
    assert!(w.r22.gallery.is_none());
    assert_eq!(style(&w).warp.as_deref(), Some("textWave1"));
    assert!(w.action("r22.transform"));
    pick(&mut w, "r22.warp:none");
    assert!(style(&w).warp.is_none());
}

#[test]
fn wordart_and_effects_open_as_thumbnails() {
    let mut w = window_with_shape();
    assert!(w.action("shape.wordArt"));
    assert!(w.menu.is_none());
    assert_eq!(
        w.r22.gallery.as_ref().map(|g| g.cells().len()),
        Some(bo_engine::doc_shapestyle::WORDART.len())
    );
    pick(&mut w, "r20.wa:1");
    // Shape Effects › Shadow: No Shadow, outer and inner.
    assert!(w.action("r20.menu:shape.effects/shadow"));
    let n = w.r22.gallery.as_ref().map(|g| g.cells().len());
    assert_eq!(n, Some(1 + bo_engine::doc_picstyle::SHADOWS.len()));
    pick(&mut w, "r20.fx:shadow=outerBottom");
    let fx = style(&w).effects.expect("effects");
    assert!(fx.shadow.is_some());
    // Escape and a press outside close a gallery.
    assert!(w.action("r20.menu:shape.effects/glow"));
    w.handle(
        Input::Key(crate::Key::Escape, crate::Mods::default()),
        &Fixed,
    );
    assert!(w.r22.gallery.is_none());
    assert!(w.action("r20.menu:shape.effects/bevel"));
    w.handle(
        Input::Press {
            x: 2.0,
            y: 1090.0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    assert!(w.r22.gallery.is_none());
}

/// Window position of page point (`x`, `y`) on page 0.
fn at_page(w: &DocWindow, x: f32, y: f32) -> (f32, f32) {
    let slot = w
        .view
        .slots()
        .into_iter()
        .find(|s| s.index == 0)
        .expect("page 0");
    let k = w.view.scale();
    (slot.x as f32 + x * k, slot.y as f32 + y * k)
}

fn press(w: &mut DocWindow, (x, y): (f32, f32)) {
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
    w.handle(Input::Tick, &Fixed);
}

/// A press on a text box's border selects the shape (Shape Format comes up); a press inside
/// puts the caret in its text.
#[test]
fn press_on_a_text_box_border_selects_the_shape() {
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
    w.run_interface_command("text:Body ", &Fixed);
    let r = w
        .view
        .exec(bo_engine::edit::Command::InsertShape {
            preset: "rect".into(),
            text_box: true,
            width: 150.0,
            height: 60.0,
        })
        .expect("inserted");
    let d = r.drawing.expect("frame");
    w.view.exec(bo_engine::edit::Command::CloseTextBox);
    w.handle(Input::Tick, &Fixed);
    let pt = at_page(&w, d.x + 40.0, d.y + 0.5);
    press(&mut w, pt);
    assert!(w.view.story().is_none(), "the text box stays closed");
    let shape = w
        .view
        .engine()
        .session()
        .and_then(bo_edit::Session::tools_shape);
    assert!(shape.is_some(), "the shape is selected");
    assert!(w.context.shape, "Shape Format is shown");
    let pt = at_page(&w, d.x + 40.0, d.y + 30.0);
    press(&mut w, pt);
    assert_eq!(w.view.story().map(|s| s.kind), Some("textBox"));
}

#[test]
fn text_box_options_and_rotate_with_shape_commands() {
    let mut w = window_with_shape();
    assert!(w.view.picture_command(&json!({
        "cmd": "shapeTextLayout", "autoFit": true, "wrap": false
    })));
    let st = style(&w);
    assert!(st.auto_fit && st.no_wrap);
    assert!(w
        .view
        .picture_command(&json!({"cmd": "shapeGradient", "variation": "dark", "index": 1})));
    assert!(w
        .view
        .picture_command(&json!({"cmd": "shapeRotateWithShape", "on": false})));
    assert_eq!(style(&w).rot_with_shape, Some(false));
}
