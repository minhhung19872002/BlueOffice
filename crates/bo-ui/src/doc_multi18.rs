//! BlueDoc round 18 (docs/ui/REPORT.md "Vòng 108"): one document in several windows (View ›
//! New Window). The host keeps one [`Engine`] per document and hands it to the window being
//! used; each [`DocView`] keeps its own selection, scroll and page images. A child module of
//! the document view.
use super::DocView;
use bo_engine::edit::Command;
use bo_engine::Engine;

impl DocView {
    /// Put `engine` in this view and return the one it held (the host moves the document's
    /// engine to the window being used; the other windows hold an empty engine meanwhile).
    pub fn replace_engine(&mut self, engine: Engine) -> Engine {
        std::mem::replace(&mut self.engine, engine)
    }

    /// Exchange engines with `other` (see [`DocView::replace_engine`]).
    pub fn swap_engine(&mut self, other: &mut DocView) {
        std::mem::swap(&mut self.engine, &mut other.engine);
    }

    /// Version of the document in the engine this view holds (0 without a document).
    pub fn doc_version(&self) -> u64 {
        self.engine.session().map_or(0, bo_edit::Session::version)
    }

    /// The engine came back to this view after another window used it: restore this view's
    /// selection (kept inside the document when the other window changed it) and bring the
    /// pages, caret and state up to date. Page images are compared with the new display
    /// lists, so only what changed is drawn again. `changed`: the document was edited since.
    pub fn adopt_engine(&mut self, changed: bool) {
        if let Some(selection) = self.selection {
            let _ = self.engine.command(Command::SetSelection { selection });
        }
        if !changed {
            return;
        }
        if let Ok(state) = self.engine.command(Command::State) {
            self.apply(&state);
        }
        self.stale.extend(self.shown.keys().copied());
        self.thumbs_stale.extend(self.thumbs_shown.keys().copied());
        self.sweep.next = 0;
    }

    /// Scroll by `dy` device pixels (Synchronous Scrolling follows another window); `true`
    /// when the view moved.
    pub fn scroll_by(&mut self, dy: f32) -> bool {
        let before = self.scroll;
        self.scroll += dy;
        self.clamp_scroll();
        (self.scroll - before).abs() > f32::EPSILON
    }

    /// The vertical scroll position in device pixels.
    pub fn scroll_y(&self) -> f32 {
        self.scroll
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bo_engine::nav::Motion;

    fn engine(text: &str) -> Engine {
        let mut e = Engine::with_bundled_fonts();
        e.load(bo_engine::plain_text_document(text), "t.docx")
            .expect("load");
        e
    }

    fn text(v: &DocView) -> String {
        v.engine()
            .document()
            .map(|d| {
                bo_model::index::body_paragraphs(d)
                    .iter()
                    .filter_map(|p| bo_model::index::para_at(d, p))
                    .map(bo_model::index::para_index_text)
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default()
    }

    #[test]
    fn two_views_share_one_engine_with_their_own_selection() {
        let mut a = DocView::new(engine("Hello world\nSecond line")).expect("view a");
        // The second window is built on the document's engine; the first one gets it back.
        let real = a.replace_engine(Engine::new());
        let mut b = DocView::new(real).expect("view b");
        b.swap_engine(&mut a);
        a.adopt_engine(false);
        // Window A goes to the end of the document and types.
        assert!(a
            .exec(Command::Move {
                motion: Motion::DocEnd,
                extend: false
            })
            .is_some());
        let v0 = a.doc_version();
        assert!(a.exec(Command::InsertText { text: "!".into() }).is_some());
        assert!(a.doc_version() > v0);
        let sel_a = a.selection;
        // Window B takes the engine: its caret is still at the start, and it sees the edit.
        b.swap_engine(&mut a);
        b.adopt_engine(true);
        assert_ne!(b.selection, sel_a);
        assert!(text(&b).ends_with("line!"), "{}", text(&b));
        // B types at its own caret (the start), A gets the engine back with its selection.
        assert!(b.exec(Command::InsertText { text: ">".into() }).is_some());
        a.swap_engine(&mut b);
        a.adopt_engine(true);
        assert!(text(&a).starts_with(">Hello"), "{}", text(&a));
        assert_eq!(
            a.selection.map(|s| s.focus.para),
            sel_a.map(|s| s.focus.para)
        );
        // Scrolling one view leaves the other where it was.
        assert!(!a.scroll_by(-50.0));
        assert!(a.scroll_y().abs() < f32::EPSILON);
    }
}
