//! Tests of the BlueSlide editing area (headless: a recording page store).
use super::*;

#[derive(Default)]
struct Store {
    full: Vec<usize>,
    bands: Vec<(usize, [u32; 4])>,
    removed: Vec<usize>,
}

impl PageStore for Store {
    fn update(
        &mut self,
        index: usize,
        _: &PageList,
        _: &Engine,
        _: f32,
        band: Option<[u32; 4]>,
    ) -> Result<(), String> {
        match band {
            Some(b) => self.bands.push((index, b)),
            None => self.full.push(index),
        }
        Ok(())
    }
    fn remove(&mut self, index: usize) {
        self.removed.push(index);
    }
}

pub(crate) fn open(name: &str) -> SlideView {
    let mut engine = Engine::with_bundled_fonts();
    let path = format!(
        "{}/../../tests/corpus/slide/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    engine.open(&std::fs::read(path).unwrap(), name).unwrap();
    let mut v = SlideView::new(engine).unwrap();
    v.set_display_scale(1.25);
    // The reference window's editing area at 125 %: 1481 × 815 device pixels.
    v.set_frame(0.0, 0.0, 1481.0, 815.0);
    v
}

fn ctrl(c: char) -> Input {
    Input::Key(
        Key::Letter(c),
        Mods {
            ctrl: true,
            ..Mods::default()
        },
    )
}

fn key(k: Key) -> Input {
    Input::Key(k, Mods::default())
}

/// Click at a slide point.
fn click_slide(v: &mut SlideView, x: f64, y: f64) {
    let (px, py) = v.to_px(x, y);
    v.handle(Input::Press {
        x: px,
        y: py,
        count: 1,
        shift: false,
    });
    v.handle(Input::Release { x: px, y: py });
}

#[test]
fn layout_matches_the_reference_at_125_percent() {
    let v = open("slide-text-bullets.pptx");
    assert_eq!(v.canvas_rect(), [235.0, 0.0, 1227.0, 769.0]);
    assert!((v.fit_zoom() - 0.75).abs() < 1e-6, "{}", v.fit_zoom());
    // Slide image at (258, 290) of the reference window, whose area starts at (9, 244).
    assert_eq!(v.slide_origin(), (249.0, 46.0));
    let t = v.thumb_rect(0);
    assert_eq!([t[0], t[2], t[3]], [49.0, 156.0, 88.0]);
    assert_eq!(v.thumb_rect(1)[1] - t[1], 115.0, "thumbnail pitch");
    assert_eq!(v.notes_rect().map(|r| r[3]), Some(45.0));
}

#[test]
fn opening_draws_the_slide_and_the_thumbnails_once() {
    let mut v = open("slide-text-bullets.pptx");
    let mut store = Store::default();
    v.prepare(&mut store).unwrap();
    assert_eq!(store.full.len(), 1 + v.slide_count());
    assert!(store.full.contains(&v.ids[0]));
    store.full.clear();
    v.prepare(&mut store).unwrap();
    assert!(store.full.is_empty() && store.bands.is_empty());
}

#[test]
fn typing_in_a_placeholder_redraws_bands_only() {
    let mut v = open("slide-text-bullets.pptx");
    let mut store = Store::default();
    v.prepare(&mut store).unwrap();
    store.full.clear();
    // The title placeholder of the deck's first slide sits in the middle of the slide.
    let (w, h) = (f64::from(v.slide_size.0), f64::from(v.slide_size.1));
    click_slide(&mut v, w / 2.0, h * 0.42);
    let te = v.text_edit().cloned().expect("text editing");
    assert!(te.caret.is_some());
    assert!(v.handle(Input::Text("X".into())));
    let after = v.text_edit().unwrap().text.clone();
    assert_eq!(utf16_len(&after), utf16_len(&te.text) + 1);
    v.prepare(&mut store).unwrap();
    assert!(store.full.is_empty(), "no full redraw: {:?}", store.full);
    let id = v.ids[0];
    assert!(store.bands.iter().any(|(k, _)| *k == id));
    assert!(store.bands.iter().any(|(k, _)| *k == THUMB_KEY + id));
    // Backspace takes the letter away again.
    assert!(v.handle(key(Key::Backspace)));
    assert_eq!(v.text_edit().unwrap().text, te.text);
    // Typing then undo restores the slide.
    let before = v.engine().slide(0).unwrap().clone();
    v.handle(Input::Text("Y".into()));
    assert_ne!(v.engine().slide(0).unwrap(), &before);
    v.handle(ctrl('z'));
    assert_eq!(v.engine().slide(0).unwrap(), &before, "undo");
    // Ctrl+B in the text toggles bold of the selection.
    if v.text_edit().is_none() {
        click_slide(&mut v, w / 2.0, h * 0.42);
    }
    v.handle(ctrl('a'));
    let bold = v.format().bold;
    v.handle(ctrl('b'));
    assert_eq!(v.format().bold, !bold);
    // Escape leaves the text, the shape stays selected; a second Escape deselects.
    v.handle(key(Key::Escape));
    assert!(v.text_edit().is_none());
    assert_eq!(v.selection().len(), 1);
    v.handle(key(Key::Escape));
    assert!(v.selection().is_empty());
}

#[test]
fn arrows_and_enter_inside_text() {
    let mut v = open("slide-text-bullets.pptx");
    let (w, h) = (f64::from(v.slide_size.0), f64::from(v.slide_size.1));
    click_slide(&mut v, w / 2.0, h * 0.42);
    v.handle(key(Key::End));
    let end = v.text_edit().unwrap().end;
    v.handle(key(Key::Left));
    assert_eq!(v.text_edit().unwrap().end, end - 1);
    v.handle(Input::Key(
        Key::Left,
        Mods {
            shift: true,
            ..Mods::default()
        },
    ));
    let te = v.text_edit().unwrap();
    assert_eq!((te.start, te.end), (end - 1, end - 2));
    assert!(!te.rects.is_empty(), "selection highlight");
    v.handle(key(Key::Enter));
    assert!(v.text_edit().unwrap().text.contains('\n'));
}

#[test]
fn new_slide_and_navigation_reuse_kept_images() {
    let mut v = open("slide-text-bullets.pptx");
    let mut store = Store::default();
    v.prepare(&mut store).unwrap();
    let n = v.slide_count();
    assert!(v.handle(ctrl('m')));
    assert_eq!(v.slide_count(), n + 1);
    assert_eq!(v.current(), 1);
    store.full.clear();
    v.prepare(&mut store).unwrap();
    let new_id = v.ids[1];
    assert!(store.full.contains(&new_id), "main image of the new slide");
    assert!(store.full.contains(&(THUMB_KEY + new_id)));
    assert_eq!(store.full.len(), 2, "other slides keep their images");
    // Back: the first slide's image is kept.
    v.handle(key(Key::PageUp));
    assert_eq!(v.current(), 0);
    store.full.clear();
    v.prepare(&mut store).unwrap();
    assert!(store.full.is_empty(), "kept image shown at once");
    // Prefetch draws the neighbours ahead.
    while v.prefetch(&mut store).unwrap() {}
    v.handle(key(Key::PageDown));
    v.handle(key(Key::PageDown));
    assert_eq!(v.current(), 2);
    store.full.clear();
    v.prepare(&mut store).unwrap();
    assert!(store.full.is_empty(), "prefetched: {:?}", store.full);
    // Delete the new slide from the thumbnails pane.
    let [tx, ty, tw, th] = v.thumb_rect(1);
    v.handle(Input::Press {
        x: tx + tw / 2.0,
        y: ty + th / 2.0,
        count: 1,
        shift: false,
    });
    v.handle(Input::Release {
        x: tx + tw / 2.0,
        y: ty + th / 2.0,
    });
    assert_eq!(v.focus(), Focus::Thumbs);
    assert_eq!(v.current(), 1);
    assert!(v.handle(key(Key::Delete)));
    assert_eq!(v.slide_count(), n);
    v.prepare(&mut store).unwrap();
    assert!(store.removed.contains(&new_id));
    v.handle(ctrl('z'));
    assert_eq!(v.slide_count(), n + 1, "undo brings the slide back");
}

#[test]
fn slide_show_steps_and_ends() {
    let mut v = open("slide-text-bullets.pptx");
    v.set_window(1920.0, 1080.0);
    let mut store = Store::default();
    assert!(v.handle(key(Key::F(5))));
    assert_eq!(v.show_slide(), Some(0));
    v.prepare(&mut store).unwrap();
    assert!(store.full.contains(&(SHOW_KEY + v.ids[0])));
    v.handle(key(Key::Right));
    assert_eq!(v.show_slide(), Some(1));
    v.handle(Input::Press {
        x: 5.0,
        y: 5.0,
        count: 1,
        shift: false,
    });
    assert_eq!(v.show_slide(), Some(2));
    v.handle(key(Key::Right));
    assert!(v.showing() && v.show_slide().is_none(), "end screen");
    v.handle(key(Key::Right));
    assert!(!v.showing(), "a step past the end screen leaves");
    v.handle(Input::Key(
        Key::F(5),
        Mods {
            shift: true,
            ..Mods::default()
        },
    ));
    v.handle(key(Key::Left));
    v.handle(key(Key::Escape));
    assert!(!v.showing());
}

#[test]
fn notes_pane_takes_typing() {
    let mut v = open("slide-text-bullets.pptx");
    let r = v.notes_rect().unwrap();
    v.handle(Input::Press {
        x: r[0] + 50.0,
        y: r[1] + 10.0,
        count: 1,
        shift: false,
    });
    assert_eq!(v.focus(), Focus::Notes);
    v.handle(Input::Text("ab".into()));
    v.handle(key(Key::Backspace));
    assert!(v.notes().ends_with('a'), "{:?}", v.notes());
}

#[test]
fn text_offsets_are_utf16() {
    let s = "a\u{1F600}b c";
    assert_eq!(step_char(s, 1, true), 3);
    assert_eq!(step_char(s, 3, false), 1);
    assert_eq!(byte_at(s, 3), 5);
    assert_eq!(step_word("ab cd", 0, true), 3);
    assert_eq!(step_word("ab cd", 5, false), 3);
    assert_eq!(word_at("ab cd", 1), (0, 3));
}
