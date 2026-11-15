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
    let mut v = open("slide-text-bullets.pptx");
    // Re-measured 2026-11-15: the reference's editing area runs from y 236 (below the
    // ribbon) to the status bar line at 1060 of a 1500 x 1102 capture: 824 px.
    v.set_frame(0.0, 0.0, 1481.0, 824.0);
    assert_eq!(v.canvas_rect(), [235.0, 0.0, 1227.0, 777.0]);
    assert!((v.fit_zoom() - 0.75).abs() < 1e-6, "{}", v.fit_zoom());
    // Slide image at (258, 286) of the reference window, whose area starts at (9, 236).
    assert_eq!(v.slide_origin(), (249.0, 50.0));
    let t = v.thumb_rect(0);
    assert_eq!([t[0], t[2], t[3]], [49.0, 156.0, 88.0]);
    assert_eq!(v.thumb_rect(1)[1] - t[1], 115.0, "thumbnail pitch");
    assert_eq!(v.notes_rect().map(|r| r[3]), Some(46.0));
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
    assert_eq!(v.selection().len(), 0);
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

/// Painter recording the slide images drawn by the show (key, destination, opacity).
#[derive(Default)]
struct Rec {
    parts: Vec<(usize, [f32; 4], f32)>,
}

impl Painter for Rec {
    fn clear(&mut self, _: Rgba) {}
    fn fill_rect(&mut self, _: f32, _: f32, _: f32, _: f32, _: Rgba) {}
    fn page(&mut self, index: usize, x: i32, y: i32) {
        self.parts
            .push((index, [x as f32, y as f32, 0.0, 0.0], 1.0));
    }
    fn text(&mut self, _: &str, _: f32, _: f32, _: TextStyle, _: Rgba) {}
    fn path(&mut self, _: &[PathSeg], _: Option<Rgba>, _: Option<(Rgba, f32)>) {}
    fn push_clip(&mut self, _: f32, _: f32, _: f32, _: f32) {}
    fn pop_clip(&mut self) {}
    fn page_part(&mut self, index: usize, _: [f32; 4], dest: [f32; 4], opacity: f32) {
        self.parts.push((index, dest, opacity));
    }
}

fn show_frame(v: &mut SlideView, ms: f64) -> Rec {
    v.set_clock(ms);
    v.prepare(&mut Store::default()).unwrap();
    let mut rec = Rec::default();
    v.paint(&mut rec, 12.0);
    rec
}

#[test]
fn show_plays_the_transition_then_stops_animating() {
    let mut v = open("slide-anim-transitions.pptx");
    v.set_window(1600.0, 900.0);
    v.set_clock(1000.0);
    v.start_show(0);
    let info = v
        .show
        .as_ref()
        .and_then(|s| s.info.clone())
        .expect("show info");
    let t = info
        .transition
        .clone()
        .expect("first slide has a transition");
    assert!(v.animating(), "{t:?}");
    // Half way through: the new slide is drawn part way in (offset or translucent).
    let half = show_frame(&mut v, 1000.0 + f64::from(t.dur_ms) / 2.0);
    let key = SHOW_KEY + v.ids[0];
    let drawn: Vec<_> = half.parts.iter().filter(|p| p.0 == key).collect();
    assert!(!drawn.is_empty(), "{:?}", half.parts);
    let centred = (1600.0 - drawn[0].1[2]) / 2.0;
    let moving = drawn.iter().any(|(_, d, o)| {
        *o < 0.99 || (d[0] - centred).abs() > 1.0 || d[3] < 899.0 && d[3] < drawn[0].1[3]
    });
    assert!(moving || t.kind == "cut", "{t:?} {drawn:?}");
    // Done: the slide is still, no more frames needed.
    let end = show_frame(&mut v, 1000.0 + f64::from(t.dur_ms) + 1.0);
    assert!(!v.animating());
    assert!(end
        .parts
        .iter()
        .any(|(k, _, o)| *k == key && (*o - 1.0).abs() < 1e-6));
    // A click goes to the next slide.
    v.handle(Input::Key(Key::Right, Mods::default()));
    assert_eq!(v.show_slide(), Some(1));
}

#[test]
fn show_builds_entrance_effects_click_by_click() {
    let mut v = open("slide-anim-fade-appear.pptx");
    v.set_window(1600.0, 900.0);
    v.set_clock(0.0);
    v.start_show(0);
    v.set_clock(100_000.0);
    let info = v
        .show
        .as_ref()
        .and_then(|s| s.info.clone())
        .expect("show info");
    assert_ne!(info.clicks, 0);
    let first = info
        .animations
        .iter()
        .find(|a| a.click == 1)
        .expect("an effect on the first click")
        .clone();
    let layer = first.layer.expect("layer");
    // Before the click an entrance effect's shape is hidden.
    let looks = crate::slide_show::layer_looks(&info, 0, f64::INFINITY);
    if first.class == "entr" {
        assert!(!looks[layer].visible);
    }
    // Click: the effect runs, then the shape stays shown.
    v.handle(Input::Key(Key::Right, Mods::default()));
    assert!(v.animating());
    let mid = f64::midpoint(f64::from(first.start_ms), f64::from(first.end_ms));
    let looks = crate::slide_show::layer_looks(&info, 1, mid);
    if first.effect == "fade" {
        let o = looks[layer].opacity;
        assert!(o > 0.05 && o < 0.95, "{:?}", looks[layer]);
    }
    let rec = show_frame(&mut v, 100_000.0 + f64::from(first.end_ms) + 10.0);
    assert!(!v.animating());
    let looks = crate::slide_show::layer_looks(&info, 1, f64::INFINITY);
    assert!(looks[layer].visible);
    // The slide is drawn from its layers.
    assert!(rec
        .parts
        .iter()
        .any(|(k, _, _)| *k >= LAYER_KEY && *k < LAYER_KEY + 64));
    // Back: the effect is undone at once.
    v.handle(Input::Key(Key::Left, Mods::default()));
    assert_eq!(v.show.as_ref().map(|s| s.step), Some(0));
    assert!(!v.animating());
}
