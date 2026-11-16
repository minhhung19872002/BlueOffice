//! BlueDoc Insert › Symbol gallery.
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

/// The document model as text (enough to find an inserted character).
fn doc_text(w: &DocWindow) -> String {
    format!("{:?}", w.view.engine().document())
}

#[test]
fn symbol_gallery_inserts_and_remembers_the_symbol() {
    let mut w = window();
    w.action("insert.symbol");
    let at = w
        .symbol_grid
        .as_ref()
        .and_then(|g| g.center_of('π'))
        .expect("gallery with pi");
    press(&mut w, at);
    assert!(w.symbol_grid.is_none(), "closes after picking");
    assert!(doc_text(&w).contains('π'));
    assert_eq!(w.recent_symbols[0], 'π', "most recent first");
    assert_eq!(w.recent_symbols.len(), 20);
    // More Symbols… shows the larger grid in place, with Vietnamese letters.
    w.action("insert.symbol");
    let more = w
        .symbol_grid
        .as_ref()
        .map(SymbolGrid::more_center)
        .expect("open");
    press(&mut w, more);
    let g = w.symbol_grid.as_ref().expect("still open");
    assert!(g.more);
    let at = g.center_of('₫').expect("dong sign");
    press(&mut w, at);
    assert!(doc_text(&w).contains('₫'));
    w.action("insert.symbol");
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(w.symbol_grid.is_none());
}
