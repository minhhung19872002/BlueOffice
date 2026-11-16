//! BlueDoc comments pane: threads with replies, resolve, delete.
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

/// A document with one commented word.
fn commented() -> DocWindow {
    let mut w = window();
    w.run_interface_command("text:Hello world", &Fixed);
    let c = catalog::from_json(json!({ "cmd": "selectAll" })).expect("selectAll");
    w.view.run_command(c);
    let c = catalog::from_json(json!({ "cmd": "addComment", "text": "Check this sentence" }))
        .expect("addComment");
    w.view.run_command(c);
    w.handle(Input::Tick, &Fixed);
    w
}

#[test]
fn show_comments_opens_the_pane_beside_the_document() {
    let mut w = commented();
    let right = w.doc_right();
    w.run_interface_command("tab:review", &Fixed);
    w.handle(Input::Tick, &Fixed);
    assert!(
        w.ribbon
            .rect_of(RibbonHit::Command("review.showComments"))
            .is_some(),
        "Show Comments on the Review tab"
    );
    w.action("review.showComments");
    w.handle(Input::Tick, &Fixed);
    let pane = w.comments_pane.as_ref().expect("pane shown");
    assert_eq!(pane.thread_ids().len(), 1);
    assert!(w.doc_right() < right, "document area narrows");
    // The proofing pane takes the same place: one at a time.
    w.action("review.spellingDialog");
    assert!(w.comments_pane.is_none());
    w.action("review.showComments");
    assert!(w.spell_pane.is_none());
    w.action("review.showComments");
    assert!(w.comments_pane.is_none(), "toggles off");
}

#[test]
fn reply_resolve_and_delete_from_the_pane() {
    let mut w = commented();
    w.action("review.showComments");
    w.handle(Input::Tick, &Fixed);
    let id = w.view.comments()[0].id;
    // Choose the thread: its buttons appear.
    let at = w
        .comments_pane
        .as_ref()
        .and_then(|p| p.card_center(id))
        .expect("card");
    press(&mut w, at);
    w.handle(Input::Tick, &Fixed);
    let reply = w
        .comments_pane
        .as_ref()
        .and_then(|p| p.button_center(id, 0))
        .expect("reply button");
    press(&mut w, reply);
    assert!(w.dialog.is_some(), "reply dialog");
    w.handle(Input::Text("Done".into()), &Fixed);
    w.dialog_button(DialogButton::Ok);
    w.handle(Input::Tick, &Fixed);
    assert!(w.dialog.is_none());
    assert!(w.view.comments().iter().any(|c| c.parent == Some(id)));
    assert_eq!(
        w.comments_pane.as_ref().map(|p| p.thread_ids().len()),
        Some(1),
        "a reply stays in its thread"
    );
    let resolve = w
        .comments_pane
        .as_ref()
        .and_then(|p| p.button_center(id, 1))
        .expect("resolve button");
    press(&mut w, resolve);
    w.handle(Input::Tick, &Fixed);
    assert!(w.view.comments().iter().any(|c| c.id == id && c.done));
    let delete = w
        .comments_pane
        .as_ref()
        .and_then(|p| p.button_center(id, 2))
        .expect("delete button");
    press(&mut w, delete);
    w.handle(Input::Tick, &Fixed);
    assert_eq!(w.view.comments().len(), 0);
    assert_eq!(
        w.comments_pane.as_ref().map(|p| p.thread_ids().len()),
        Some(0)
    );
}
