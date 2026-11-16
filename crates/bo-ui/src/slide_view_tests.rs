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
    // The thumbnail follows when idle (prefetch), off the keystroke's path.
    assert!(!store.bands.iter().any(|(k, _)| *k == THUMB_KEY + id));
    assert!(v.prefetch(&mut store).unwrap());
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
    // Triple click: the paragraph with its mark.
    assert_eq!(para_at("ab\ncd ef\ngh", 4), (3, 9));
    assert_eq!(para_at("ab\ncd", 1), (0, 3));
    assert_eq!(para_at("ab\ncd", 4), (3, 5));
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

/// The view with every shape of slide 1 given `spec`, showing from slide 1.
fn show_with(spec: bo_engine::slide_edit::show::AnimSpec) -> (SlideView, ShowOut) {
    let mut v = open("slide-text-bullets.pptx");
    v.run(SlideCommand::SelectAll);
    assert!(v.run(SlideCommand::AddAnimation { spec }));
    v.set_window(1600.0, 900.0);
    v.set_clock(0.0);
    v.start_show(0);
    v.set_clock(100_000.0);
    let info = v
        .show
        .as_ref()
        .and_then(|s| s.info.clone())
        .expect("show info");
    (v, info)
}

fn spec(effect: &str, class: &str) -> bo_engine::slide_edit::show::AnimSpec {
    bo_engine::slide_edit::show::AnimSpec {
        effect: Some(effect.into()),
        class: Some(class.into()),
        ..Default::default()
    }
}

#[test]
fn show_spins_and_uncovers_with_filters() {
    use crate::slide_show::{effect_look, Look};
    let (_, info) = show_with(spec("spin", "emph"));
    let a = info.animations.first().expect("spin").clone();
    let slide = (f64::from(info.width), f64::from(info.height));
    let mid = f64::from(a.dur_ms) / 2.0;
    let shown = crate::slide_show::layer_looks(&info, 0, 0.0)[0];
    let look = effect_look(&a, mid, shown, slide, None);
    assert!(look.rot.abs() > 10.0, "{look:?}");
    let (mut v, info) = show_with(spec("blinds", "entr"));
    let a = info.animations.first().expect("blinds").clone();
    let mid = f64::from(a.dur_ms) / 2.0;
    let hidden = Look {
        visible: false,
        ..shown
    };
    let look = effect_look(&a, mid, hidden, slide, None);
    let (rev, q) = look.reveal.expect("a reveal pattern");
    assert_eq!(rev, crate::slide_reveal::Reveal::Blinds { vert: false });
    assert!(q > 0.2 && q < 0.8, "{q}");
    // Mid way the layer is drawn as six bands.
    v.handle(Input::Key(Key::Right, Mods::default()));
    let rec = show_frame(&mut v, 100_000.0 + f64::from(a.start_ms) + mid);
    let bands = rec
        .parts
        .iter()
        .filter(|(k, _, _)| *k >= LAYER_KEY && *k < LAYER_KEY + 64)
        .count();
    assert!(bands >= 6, "{:?}", rec.parts);
}

#[test]
fn show_builds_text_letter_by_letter() {
    use crate::slide_show::{layer_images, layer_mode, pieces, LayerMode, PieceImage};
    let (_, info) = show_with(bo_engine::slide_edit::show::AnimSpec {
        text_by: Some("letter".into()),
        ..spec("fade", "entr")
    });
    let li = (0..info.layers.len())
        .find(|li| layer_mode(&info, *li) == LayerMode::Text)
        .expect("a layer cut into letters");
    let a = info
        .animations
        .iter()
        .find(|a| a.layer == Some(li))
        .expect("effect")
        .clone();
    let mid = f64::midpoint(f64::from(a.start_ms), f64::from(a.end_ms));
    let pcs: Vec<_> = pieces(&info, a.click, mid)
        .into_iter()
        .filter(|p| p.image == PieceImage::Pieces(li))
        .collect();
    assert!(pcs.len() > 3);
    // Half way: the first letters are in, the last ones not yet.
    assert!(pcs.first().is_some_and(|p| p.look.visible));
    assert!(pcs
        .last()
        .is_some_and(|p| !p.look.visible || p.look.opacity < 0.5));
    let (_, letters) = layer_images(&info, li);
    assert!(letters.is_some_and(|l| !l.commands.is_empty()));
}

#[test]
fn show_builds_paragraph_by_paragraph() {
    use crate::slide_show::{layer_mode, pieces, LayerMode, PieceImage};
    // A deck whose effects build shapes element by element (paragraphs, chart, SmartArt).
    let mut v = open("slide-edit-anim-build-elements.pptx");
    let found = (0..v.slide_count()).find_map(|i| {
        let info = v
            .engine
            .slide_command(SlideCommand::Show { slide: i })
            .ok()
            .and_then(|r| r.show)?;
        let li = (0..info.layers.len()).find(|li| layer_mode(&info, *li) == LayerMode::Parts)?;
        Some((info, li))
    });
    let (info, li) = found.expect("a layer built by parts");
    let shown = |step| {
        pieces(&info, step, f64::INFINITY)
            .into_iter()
            .filter(|p| p.image == PieceImage::Pieces(li))
            .map(|p| p.look.visible)
            .collect::<Vec<_>>()
    };
    let first_click = info
        .animations
        .iter()
        .filter(|a| a.layer == Some(li))
        .map(|a| a.click)
        .min()
        .unwrap_or(0);
    let before = shown(first_click.saturating_sub(1));
    let after = shown(first_click);
    // Pieces come in click by click: some are still hidden before the first click, more
    // show after it, and not all of them at once.
    let count = |v: &[bool]| v.iter().filter(|s| **s).count();
    assert!(before.len() > 1, "{before:?}");
    assert!(count(&before) < before.len(), "{before:?}");
    assert!(count(&after) > count(&before), "{before:?} {after:?}");
    let all = shown(info.clicks);
    assert_eq!(count(&all), all.len(), "{all:?}");
}

#[test]
fn show_morphs_matched_shapes() {
    let mut v = open("slide-text-bullets.pptx");
    assert!(v.run(SlideCommand::DuplicateSlide { index: Some(0) }));
    v.go_to(1);
    v.run(SlideCommand::SelectAll);
    v.run(SlideCommand::Transform {
        transform: bo_engine::slide_edit::Transform::Move { dx: 80.0, dy: 40.0 },
        snap: false,
        tolerance: 0.0,
        preview: false,
    });
    assert!(v.run(SlideCommand::SetTransition {
        slides: None,
        all: false,
        transition: bo_engine::slide_edit::show::TransitionSpec {
            kind: "morph".into(),
            dur_ms: Some(2000),
            ..Default::default()
        },
    }));
    v.set_window(1600.0, 900.0);
    v.set_clock(0.0);
    v.start_show(0);
    v.set_clock(10_000.0);
    v.handle(Input::Key(Key::Right, Mods::default()));
    let m = v
        .show
        .as_ref()
        .and_then(|s| s.info.as_ref())
        .and_then(|i| i.morph.clone())
        .expect("morph data");
    assert_ne!(m.pairs.len(), 0);
    let rec = show_frame(&mut v, 11_000.0);
    let morph: Vec<_> = rec
        .parts
        .iter()
        .filter(|(k, _, _)| *k >= MORPH_KEY && *k < MORPH_KEY + 8)
        .collect();
    assert!(!morph.is_empty(), "{:?}", rec.parts);
}

#[test]
fn show_keys_blank_jump_and_draw() {
    use super::show_tools::{Pointer, Screen};
    let mut v = open("slide-anim-transitions.pptx");
    v.set_window(1600.0, 900.0);
    v.set_clock(0.0);
    v.start_show(0);
    let n = v.slide_count();
    assert!(n >= 2);
    // B blanks the screen; any key brings the slide back without advancing.
    v.handle(Input::Text("b".into()));
    assert_eq!(v.show_tools().screen, Some(Screen::Black));
    let mut rec = Rec::default();
    v.paint(&mut rec, 12.0);
    v.handle(key(Key::Right));
    assert_eq!(v.show_tools().screen, None);
    assert_eq!(v.show_slide(), Some(0));
    v.handle(Input::Text(",".into()));
    assert_eq!(v.show_tools().screen, Some(Screen::White));
    v.handle(Input::Text("w".into()));
    assert_eq!(v.show_tools().screen, None);
    // Number + Enter goes to that slide.
    v.handle(Input::Text(n.to_string()));
    v.handle(key(Key::Enter));
    assert_eq!(v.show_slide(), Some(n - 1));
    v.handle(Input::Text("1".into()));
    v.handle(key(Key::Enter));
    assert_eq!(v.show_slide(), Some(0));
    // Ctrl+P: the pen draws instead of advancing; E erases; Esc back to the arrow.
    v.handle(ctrl('p'));
    assert_eq!(v.show_tools().pointer, Pointer::Pen);
    v.handle(Input::Press {
        x: 400.0,
        y: 300.0,
        count: 1,
        shift: false,
    });
    v.handle(Input::Drag { x: 600.0, y: 350.0 });
    v.handle(Input::Release { x: 600.0, y: 350.0 });
    assert_eq!(v.show_slide(), Some(0));
    assert_eq!(v.show_tools().strokes.get(&0).map(Vec::len), Some(1));
    let mut rec = Rec::default();
    v.paint(&mut rec, 12.0);
    v.handle(Input::Text("e".into()));
    assert_eq!(v.show_tools().strokes.len(), 0);
    v.handle(key(Key::Escape));
    assert_eq!(v.show_tools().pointer, Pointer::Arrow);
    assert!(v.showing(), "the first Esc only drops the pen");
    // Laser follows the pointer.
    v.handle(ctrl('l'));
    v.handle(Input::Move { x: 500.0, y: 400.0 });
    assert_eq!(v.show_tools().laser, Some((500.0, 400.0)));
    v.handle(key(Key::Escape));
    v.handle(key(Key::Escape));
    assert!(!v.showing());
}

fn shape_count(v: &mut SlideView) -> usize {
    v.engine_mut()
        .slide_command(SlideCommand::ShapeBoxes)
        .ok()
        .and_then(|r| r.shape_boxes)
        .map_or(0, |b| b.shapes.len())
}

fn drag_px(v: &mut SlideView, a: (f32, f32), b: (f32, f32), shift: bool) {
    v.handle(Input::Press {
        x: a.0,
        y: a.1,
        count: 1,
        shift,
    });
    v.handle(Input::Drag { x: b.0, y: b.1 });
    v.handle(Input::Release { x: b.0, y: b.1 });
}

#[test]
fn resizing_draws_the_new_size_half_see_through_over_the_slide_without_it() {
    use bo_engine::slide_edit::geom::Rect;
    let mut v = open("slide-text-bullets.pptx");
    let (sw, sh) = v.slide_points();
    let (sw, sh) = (f64::from(sw), f64::from(sh));
    assert!(v.run(SlideCommand::InsertShape {
        preset: "rect".into(),
        rect: Some(Rect {
            x: sw / 2.0,
            y: sh / 2.0,
            w: 100.0,
            h: 60.0,
        }),
    }));
    let mut store = Store::default();
    v.prepare(&mut store).unwrap();
    // Bottom-right handle, dragged 40 pt out.
    let a = v.to_px(sw / 2.0 + 100.0, sh / 2.0 + 60.0);
    let b = v.to_px(sw / 2.0 + 140.0, sh / 2.0 + 100.0);
    v.handle(Input::Press {
        x: a.0,
        y: a.1,
        count: 1,
        shift: false,
    });
    v.handle(Input::Drag { x: b.0, y: b.1 });
    store.full.clear();
    v.prepare(&mut store).unwrap();
    assert!(store.full.contains(&GHOST_KEY), "{:?}", store.full);
    assert!(v.ghost_shown());
    let mut rec = Rec::default();
    v.paint(&mut rec, 12.0);
    let id = v.ids[v.current()];
    assert!(rec
        .parts
        .iter()
        .any(|(k, _, o)| *k == GHOST_KEY && *o == 1.0));
    let part = rec
        .parts
        .iter()
        .find(|(k, _, _)| *k == id)
        .expect("the new size drawn over the slide");
    assert!((part.2 - 0.6).abs() < 1e-6);
    // The part covers the new box (140 × 100 pt from the old top-left corner).
    let (l, t) = v.to_px(sw / 2.0, sh / 2.0);
    assert!(part.1[0] <= l && part.1[1] <= t, "{part:?}");
    assert!(part.1[0] + part.1[2] >= b.0 && part.1[1] + part.1[3] >= b.1);
    v.handle(Input::Release { x: b.0, y: b.1 });
    assert!(!v.ghost_shown());
    v.prepare(&mut store).unwrap();
    assert!(store.removed.contains(&GHOST_KEY));
    // (Smart guides may snap the edge a little.)
    assert!((v.selection()[0].rect.w - 140.0).abs() < 10.0);
}

#[test]
fn tab_walks_table_cells_and_adds_a_row_at_the_end() {
    let mut v = open("slide-text-bullets.pptx");
    assert!(v.run(SlideCommand::InsertTable {
        rows: 2,
        cols: 2,
        rect: None,
    }));
    let id = v.selection()[0].id;
    assert!(v.run(SlideCommand::TextSelect {
        id: Some(id),
        start: 0,
        end: 0,
        cell: Some([0, 0]),
    }));
    v.focus = Focus::Canvas;
    v.handle(Input::Text("a".into()));
    let te = v.text_edit().cloned().unwrap();
    assert_eq!((te.cell, te.text.as_str()), (Some([0, 0]), "a"), "{te:?}");
    let cell = |v: &SlideView| v.text_edit().and_then(|t| t.cell);
    v.handle(key(Key::Tab));
    assert_eq!(cell(&v), Some([0, 1]));
    v.handle(Input::Text("b".into()));
    v.handle(Input::Key(
        Key::Tab,
        Mods {
            shift: true,
            ..Mods::default()
        },
    ));
    // Back in the first cell, its text selected.
    let te = v.text_edit().cloned().unwrap();
    assert_eq!((te.cell, te.start, te.end), (Some([0, 0]), 0, 1), "{te:?}");
    for _ in 0..3 {
        v.handle(key(Key::Tab));
    }
    assert_eq!(cell(&v), Some([1, 1]));
    // Tab in the last cell: a third row, its first cell.
    v.handle(key(Key::Tab));
    assert_eq!(cell(&v), Some([2, 0]));
    assert_eq!(
        v.state().and_then(|s| s.table.as_ref()).map(|t| t.rows),
        Some(3)
    );
}

#[test]
fn marquee_constrained_move_and_ctrl_drag_copy() {
    use bo_engine::slide_edit::geom::Rect;
    let mut v = open("slide-text-bullets.pptx");
    let (sw, sh) = v.slide_points();
    let (sw, sh) = (f64::from(sw), f64::from(sh));
    assert!(v.run(SlideCommand::InsertShape {
        preset: "rect".into(),
        rect: Some(Rect {
            x: sw - 80.0,
            y: sh - 60.0,
            w: 60.0,
            h: 40.0,
        }),
    }));
    let id = v.selection()[0].id;
    // Marquee from the empty bottom-right corner round the shape selects it alone.
    click_slide(&mut v, sw - 2.0, sh - 2.0);
    assert_eq!(v.selection().len(), 0);
    let a = v.to_px(sw - 2.0, sh - 2.0);
    let b = v.to_px(sw - 90.0, sh - 70.0);
    v.handle(Input::Press {
        x: a.0,
        y: a.1,
        count: 1,
        shift: false,
    });
    v.handle(Input::Drag { x: b.0, y: b.1 });
    assert!(v.marquee().is_some());
    let mut rec = Rec::default();
    v.paint(&mut rec, 12.0);
    v.handle(Input::Release { x: b.0, y: b.1 });
    assert!(v.marquee().is_none());
    let ids: Vec<u32> = v.selection().iter().map(|s| s.id).collect();
    assert_eq!(ids, vec![id]);
    // Shift + drag moves straight across.
    let r0 = v.selection()[0].rect;
    let c = v.to_px(r0.cx, r0.cy);
    v.set_pointer_mods(Mods {
        shift: true,
        ..Mods::default()
    });
    drag_px(&mut v, c, (c.0 - 60.0, c.1 - 9.0), true);
    let r1 = v.selection()[0].rect;
    assert!((r1.cy - r0.cy).abs() < 1e-6, "{r0:?} {r1:?}");
    assert!(r1.cx < r0.cx - 10.0);
    // Ctrl + drag leaves the shape and moves a copy (one undo step).
    let before = shape_count(&mut v);
    v.set_pointer_mods(Mods {
        ctrl: true,
        ..Mods::default()
    });
    let c = v.to_px(r1.cx, r1.cy);
    drag_px(&mut v, c, (c.0, c.1 - 50.0), false);
    assert_eq!(shape_count(&mut v), before + 1);
    let r2 = v.selection()[0].rect;
    assert_ne!(v.selection()[0].id, id);
    assert!(
        r2.cy < r1.cy - 10.0 && (r2.cx - r1.cx).abs() < 2.0,
        "{r1:?} {r2:?}"
    );
    // A click on the selected copy without moving puts the caret in its text.
    let c = v.to_px(r2.cx, r2.cy);
    v.set_pointer_mods(Mods::default());
    drag_px(&mut v, c, c, false);
    assert!(v.text_edit().is_some());
    v.handle(key(Key::Escape));
    v.handle(ctrl('z'));
    assert_eq!(shape_count(&mut v), before);
}

#[test]
fn misspelt_words_get_wavy_lines_and_tab_types_inside_a_paragraph() {
    let mut v = open("slide-text-bullets.pptx");
    let placeholder = v.state().and_then(|s| s.selection.first().map(|i| i.id));
    let id = placeholder.or_else(|| {
        v.run(SlideCommand::SelectAll);
        v.selection().first().map(|s| s.id)
    });
    let id = id.expect("a shape");
    v.run(SlideCommand::TextSelect {
        id: Some(id),
        start: 0,
        end: 0,
        cell: None,
    });
    v.handle(Input::Text("Helo wrold ".into()));
    v.spell_tick();
    assert!(!v.spell_marks().is_empty(), "wavy lines under the typos");
    let mut rec = Rec::default();
    v.paint(&mut rec, 12.0);
    // Tab inside the paragraph types a tab character; at its start it demotes.
    let before = v.text_edit().map(|t| t.text.clone()).unwrap_or_default();
    v.handle(key(Key::Tab));
    let after = v.text_edit().map(|t| t.text.clone()).unwrap_or_default();
    assert_eq!(
        after.matches('\t').count(),
        before.matches('\t').count() + 1
    );
}

#[test]
fn slide_sorter_has_its_own_zoom() {
    let mut v = open("slide-text-bullets.pptx");
    let edit = v.scale();
    v.set_mode(ViewMode::Sorter);
    assert!((v.zoom() - 1.0).abs() < 1e-6);
    assert_eq!(v.sort_rect(0)[2], 400.0, "320 logical px at 125 %");
    assert!(v.set_zoom(Some(0.5)));
    assert_eq!(v.sort_rect(0)[2], 200.0);
    assert!((v.scale() - edit).abs() < 1e-6, "the editing zoom is kept");
    v.set_mode(ViewMode::Normal);
    assert!((v.zoom() - edit / v.base_scale()).abs() < 1e-6);
}
