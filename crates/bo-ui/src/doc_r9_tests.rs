//! BlueDoc parity round 9: block selection, selection-bar drags, drag-and-drop.
use super::super::DocWindow;
use crate::{DocView, Input, Key, Mods, TextMeasure, TextStyle};
use bo_edit::{Pos, Selection};
use bo_engine::edit::Command;
use bo_engine::Engine;

struct NullStore;

impl crate::PageStore for NullStore {
    fn update(
        &mut self,
        _: usize,
        _: &bo_render::PageList,
        _: &Engine,
        _: f32,
        _: Option<[u32; 4]>,
    ) -> Result<(), String> {
        Ok(())
    }
    fn remove(&mut self, _: usize) {}
}

/// Records the text and filled shapes painted.
#[derive(Default)]
struct Rec {
    texts: Vec<String>,
    shapes: usize,
}

impl crate::Painter for Rec {
    fn clear(&mut self, _: bo_render::Rgba) {}
    fn fill_rect(&mut self, _: f32, _: f32, _: f32, _: f32, _: bo_render::Rgba) {
        self.shapes += 1;
    }
    fn page(&mut self, _: usize, _: i32, _: i32) {}
    fn text(&mut self, text: &str, _: f32, _: f32, _: TextStyle, _: bo_render::Rgba) {
        self.texts.push(text.to_owned());
    }
    fn path(
        &mut self,
        _: &[bo_render::PathSeg],
        _: Option<bo_render::Rgba>,
        _: Option<(bo_render::Rgba, f32)>,
    ) {
        self.shapes += 1;
    }
    fn push_clip(&mut self, _: f32, _: f32, _: f32, _: f32) {}
    fn pop_clip(&mut self) {}
}

#[test]
fn paragraph_marks_are_shapes_not_interface_text() {
    // Interface text is ClearType: a ¶ drawn as text had colour fringes beside the grey
    // document text (round 9). The mark is drawn with shapes in the text colour.
    let mut w = window();
    w.view.run_command(Command::ShowFormattingMarks {
        on: true,
        kinds: None,
    });
    w.handle(Input::Tick, &Fixed);
    let mut store = NullStore;
    let _ = w.view.prepare(&mut store);
    let mut rec = Rec::default();
    w.view.paint(&mut rec);
    assert!(rec.shapes > 0);
    assert!(
        !rec.texts.iter().any(|t| t.contains('\u{b6}')),
        "{:?}",
        rec.texts
    );
}

struct Fixed;
impl TextMeasure for Fixed {
    fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
        (
            text.chars().count() as f32 * style.size * 0.55,
            style.size * 1.33,
        )
    }
}

const TEXT: &str = "abcdef ghij\nabcdef ghij\nabcdef ghij\nThird paragraph with some words to move through and a long enough line so that it wraps onto a second line in the page at the default margins for sure, and a third one too because it goes on and on past the right margin.\nLast paragraph.";

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
    w.view
        .run_command(Command::InsertText { text: TEXT.into() });
    w.view.run_command(Command::SetSelection {
        selection: Selection::caret(Pos::new(0, 1)),
    });
    w
}

/// Window pixel of byte `offset` of paragraph `para` (the n-th line of the paragraph holding
/// it, vertically centred).
fn pixel_of(w: &DocWindow, para: usize, offset: usize) -> (f32, f32) {
    let layout = w.view.engine().layout().expect("layout");
    let slot = w.view.slots()[0];
    let s = w.view.scale();
    let page = &layout.pages[slot.index];
    let line = page
        .lines
        .iter()
        .find(|l| {
            l.source == bo_layout::LineSource::Body(para as u32)
                && (l.start as usize) <= offset
                && offset < (l.end as usize).max(l.start as usize + 1)
        })
        .expect("line");
    let x = line
        .carets
        .iter()
        .find(|c| c.0 as usize >= offset)
        .map_or(0.0, |c| c.1);
    (
        slot.x as f32 + x * s,
        slot.y as f32 + (line.top + line.height / 2.0) * s,
    )
}

fn sel(w: &DocWindow) -> (Pos, Pos) {
    w.r9_selection().expect("selection").ordered()
}

fn para_text(w: &DocWindow, p: usize) -> String {
    w.view
        .engine()
        .session()
        .and_then(|s| s.para_text(p))
        .unwrap_or_default()
}

fn key(w: &mut DocWindow, k: Key, mods: Mods) {
    w.handle(Input::Key(k, mods), &Fixed);
}

const CTRL: Mods = Mods {
    ctrl: true,
    shift: false,
    alt: false,
};

#[test]
fn ctrl_shift_f8_and_arrows_make_a_block_that_bold_and_delete_use() {
    let mut w = window();
    key(
        &mut w,
        Key::F(8),
        Mods {
            ctrl: true,
            shift: true,
            alt: false,
        },
    );
    for k in [Key::Down, Key::Down, Key::Right, Key::Right] {
        key(&mut w, k, Mods::default());
    }
    assert_eq!(w.view.engine().block_text(), ["bc", "bc", "bc"]);
    key(&mut w, Key::Letter('b'), CTRL);
    assert!(w.view.engine().block_active());
    assert!(w.view.format().bold);
    key(&mut w, Key::Delete, Mods::default());
    assert!(!w.view.engine().block_active());
    for p in 0..3 {
        assert_eq!(para_text(&w, p), "adef ghij");
    }
    // Esc ends a block made from the keyboard.
    key(
        &mut w,
        Key::F(8),
        Mods {
            ctrl: true,
            shift: true,
            alt: false,
        },
    );
    key(&mut w, Key::Down, Mods::default());
    assert!(w.view.engine().block_active());
    key(&mut w, Key::Escape, Mods::default());
    assert!(!w.view.engine().block_active());
}

#[test]
fn alt_drag_selects_a_block_and_a_click_ends_it() {
    let mut w = window();
    let (x0, y0) = pixel_of(&w, 0, 2);
    let (x1, y1) = pixel_of(&w, 2, 5);
    w.set_pointer_mods(Mods {
        alt: true,
        ..Mods::default()
    });
    w.handle(
        Input::Press {
            x: x0,
            y: y0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Drag { x: x1, y: y1 }, &Fixed);
    w.handle(Input::Release { x: x1, y: y1 }, &Fixed);
    assert_eq!(w.view.engine().block_text(), ["cde", "cde", "cde"]);
    let (x, y) = pixel_of(&w, 4, 3);
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
    assert!(!w.view.engine().block_active());
}

#[test]
fn selection_bar_drag_extends_line_by_line_and_by_paragraphs_after_a_double_click() {
    let mut w = window();
    let slot = w.view.slots()[0];
    let bar = slot.x as f32 + 30.0 * w.view.scale();
    let (_, y0) = pixel_of(&w, 3, 0);
    let (_, y1) = pixel_of(&w, 3, 120);
    w.handle(
        Input::Press {
            x: bar,
            y: y0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Drag { x: bar, y: y1 }, &Fixed);
    let (a, b) = sel(&w);
    // From the first line's start to the end of the second line (not to the pointer's x).
    assert_eq!(a, Pos::new(3, 0));
    assert_eq!(b.para, 3);
    assert!(b.offset > 120, "{b:?}");
    let first_line_end = {
        w.handle(Input::Drag { x: bar, y: y0 }, &Fixed);
        sel(&w).1
    };
    assert!(first_line_end.offset < 120 && first_line_end.offset > 0);
    // Dragging up from the line: from its end up to the line above's start.
    let (_, ya) = pixel_of(&w, 2, 0);
    w.handle(Input::Drag { x: bar, y: ya }, &Fixed);
    assert_eq!(sel(&w), (Pos::new(2, 0), first_line_end));
    w.handle(Input::Release { x: bar, y: ya }, &Fixed);
    // A double click selects the paragraph; dragging down adds whole paragraphs.
    let (_, y2) = pixel_of(&w, 1, 0);
    w.handle(
        Input::Press {
            x: bar,
            y: y2,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Release { x: bar, y: y2 }, &Fixed);
    w.handle(
        Input::Press {
            x: bar,
            y: y2,
            count: 2,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Drag { x: bar, y: y1 }, &Fixed);
    assert_eq!(sel(&w), (Pos::new(1, 0), Pos::new(4, 0)));
    w.handle(Input::Release { x: bar, y: y1 }, &Fixed);
}

#[test]
fn dragging_the_selection_moves_it_and_ctrl_copies() {
    let mut w = window();
    w.view.run_command(Command::SetSelection {
        selection: Selection {
            anchor: Pos::new(4, 0),
            focus: Pos::new(4, 5),
        },
    });
    let (x0, y0) = pixel_of(&w, 4, 2);
    let (x1, y1) = pixel_of(&w, 0, 7);
    w.handle(
        Input::Press {
            x: x0,
            y: y0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    // The press alone keeps the selection (it may become a drag).
    assert_eq!(sel(&w), (Pos::new(4, 0), Pos::new(4, 5)));
    w.handle(Input::Drag { x: x1, y: y1 }, &Fixed);
    w.handle(Input::Release { x: x1, y: y1 }, &Fixed);
    assert_eq!(para_text(&w, 0), "abcdef Last ghij");
    assert_eq!(para_text(&w, 4), "paragraph.");
    assert_eq!(sel(&w), (Pos::new(0, 7), Pos::new(0, 12)));
    // Ctrl at the drop copies.
    let (x0, y0) = pixel_of(&w, 0, 9);
    let (x1, y1) = pixel_of(&w, 1, 0);
    w.handle(
        Input::Press {
            x: x0,
            y: y0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Drag { x: x1, y: y1 }, &Fixed);
    w.set_release_mods(CTRL);
    w.handle(Input::Release { x: x1, y: y1 }, &Fixed);
    assert_eq!(para_text(&w, 0), "abcdef Last ghij");
    assert_eq!(para_text(&w, 1), "Last abcdef ghij");
    // A press inside the selection without a drag places the caret there.
    let (x, y) = pixel_of(&w, 1, 2);
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
    let (a, b) = sel(&w);
    assert_eq!(a, b);
}

#[test]
fn plus_and_hyphens_then_enter_make_a_table() {
    let mut engine = Engine::with_bundled_fonts();
    engine.new_document("a.docx").expect("new");
    let mut w = DocWindow::new(DocView::new(engine).expect("view"), "en-US", "a.docx");
    w.handle(Input::Scale(1.25), &Fixed);
    for c in "+-----+--------+".chars() {
        w.handle(Input::Text(c.to_string()), &Fixed);
    }
    key(&mut w, Key::Enter, Mods::default());
    let cols = w.view.format().table.as_ref().map(|t| t.cols);
    assert_eq!(cols, Some(2), "{:?}", para_text(&w, 0));
}

#[test]
fn header_paragraph_marks_are_drawn_with_show_marks() {
    let mut w = window();
    w.view.run_command(Command::EditHeaderFooter {
        page: 0,
        header: true,
    });
    w.view.run_command(Command::InsertText {
        text: "Head".into(),
    });
    w.view.run_command(Command::CloseHeaderFooter);
    w.view.run_command(Command::ShowFormattingMarks {
        on: true,
        kinds: None,
    });
    let marks = w.view.engine().formatting_marks(0);
    let first_body = w
        .view
        .engine()
        .layout()
        .and_then(|l| {
            l.pages[0]
                .lines
                .iter()
                .find(|l| matches!(l.source, bo_layout::LineSource::Body(_)))
                .map(|l| l.top)
        })
        .expect("body");
    assert!(marks
        .iter()
        .any(|m| m.top < first_body && m.kind == bo_engine::views::FormatMarkKind::Paragraph));
}
