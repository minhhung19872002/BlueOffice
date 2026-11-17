//! Tests of BlueSlide round 8: context menus with spelling corrections, the Spelling pane,
//! Insert menus and collapsed group labels, Preview / Delete comment enabled states
//! (headless).
use super::*;
use bo_engine::slide_edit::geom::Rect;
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

fn window() -> SlideWindow {
    let mut engine = Engine::with_bundled_fonts();
    let name = "slide-text-bullets.pptx";
    let path = format!(
        "{}/../../tests/corpus/slide/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    engine
        .open(&std::fs::read(path).expect("corpus deck"), name)
        .expect("open");
    let mut w = SlideWindow::new(SlideView::new(engine).expect("view"), "en-US", name);
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1481.0,
            height: 1080.0,
        },
        &Fixed,
    );
    w
}

/// A text box reading `text` (English), selected as a shape; its id.
fn text_box(w: &mut SlideWindow, text: &str) -> u32 {
    assert!(w.view.run(SlideCommand::InsertTextBox {
        text: text.into(),
        rect: Some(Rect {
            x: 100.0,
            y: 100.0,
            w: 400.0,
            h: 60.0,
        }),
    }));
    w.view.selection()[0].id
}

/// Text of shape `id` on the current slide.
fn shape_text(w: &SlideWindow, id: u32) -> String {
    let p = w.view.engine().presentation().expect("deck");
    bo_engine::slide_edit::a11y::slide_shapes(p, w.view.current())
        .into_iter()
        .find(|s| s.id == id)
        .and_then(|s| s.text)
        .unwrap_or_default()
}

fn menu_labels(w: &SlideWindow) -> Vec<String> {
    w.menu
        .as_ref()
        .map(|m| m.items().iter().map(|i| i.label.clone()).collect())
        .unwrap_or_default()
}

#[test]
fn context_menu_offers_corrections_of_a_misspelt_word() {
    let mut w = window();
    let id = text_box(&mut w, "Xin chàoo");
    w.view.run(SlideCommand::Select { ids: vec![] });
    w.view.spell_tick();
    let (from, to, _) = *w
        .view
        .spell_marks()
        .iter()
        .find(|(_, _, g)| !g)
        .expect("a red wavy line");
    let (x, y) = w.view.to_px(f64::midpoint(from[0], to[0]), from[1] - 4.0);
    assert!(w.handle(Input::Context { x, y }, &Fixed));
    let labels = menu_labels(&w);
    let i = labels
        .iter()
        .position(|l| l == "chào")
        .unwrap_or_else(|| panic!("{labels:?}"));
    assert!(
        labels.iter().any(|l| l.to_lowercase() == "ignore all"),
        "{labels:?}"
    );
    assert!(labels.contains(&"Paste".to_owned()), "{labels:?}");
    assert!(w.choose(Choice::Command(format!("ctxsp:{i}"))));
    assert_eq!(shape_text(&w, id), "Xin chào");
    // One undo step brings the word back.
    assert!(w.view.action("edit.undo"));
    assert_eq!(shape_text(&w, id), "Xin chàoo");
}

#[test]
fn context_menus_of_the_slide_and_thumbnails() {
    let mut w = window();
    w.view.run(SlideCommand::Select { ids: vec![] });
    let [cx, cy, cw, ch] = w.view.canvas_rect();
    assert!(w.handle(
        Input::Context {
            x: cx + cw - 4.0,
            y: cy + ch - 4.0
        },
        &Fixed
    ));
    let labels = menu_labels(&w);
    assert!(labels.contains(&"Paste".to_owned()), "{labels:?}");
    assert!(
        labels.iter().any(|l| l.starts_with("Format Background")),
        "{labels:?}"
    );
    let cut = w.menu.as_ref().expect("menu").items()[0].clone();
    assert!(cut.dim, "Cut is dimmed without a selection");
    w.menu = None;
    // A thumbnail: slide commands.
    let [tx, ty, tw, th] = w.view.thumb_rect(0);
    assert!(w.handle(
        Input::Context {
            x: tx + tw / 2.0,
            y: ty + th / 2.0
        },
        &Fixed
    ));
    let labels = menu_labels(&w);
    assert!(labels.contains(&"Duplicate Slide".to_owned()), "{labels:?}");
    let n = w.view.slide_count();
    assert!(w.choose(Choice::Command("slide.duplicate".into())));
    assert_eq!(w.view.slide_count(), n + 1);
}

#[test]
fn spelling_pane_walks_and_corrects_the_deck() {
    let mut w = window();
    let id = text_box(&mut w, "Xin chàoo");
    assert!(w.action("review.spellingDialog"));
    assert_eq!(w.side.kind, selpane::SideKind::Spelling);
    // Walk to our word (the corpus deck may hold other flagged words before it).
    for _ in 0..20 {
        if w.spell_shown().word.as_deref() == Some("chàoo") {
            break;
        }
        assert!(w.spelling_hit(&selpane::SideHit::IgnoreOnce));
    }
    let shown = w.spell_shown();
    assert_eq!(shown.word.as_deref(), Some("chàoo"));
    let i = shown
        .suggestions
        .iter()
        .position(|s| s == "chào")
        .unwrap_or_else(|| panic!("{shown:?}"));
    // The flagged word is selected in its shape.
    let te = w.view.text_edit().expect("editing the word");
    assert_eq!(
        (te.id, te.start.min(te.end), te.start.max(te.end)),
        (id, 4, 9)
    );
    assert!(w.spelling_hit(&selpane::SideHit::Suggestion(i)));
    assert!(w.spelling_hit(&selpane::SideHit::Change));
    assert_eq!(shape_text(&w, id), "Xin chào");
    assert_ne!(w.spell_shown().word.as_deref(), Some("chàoo"));
    // The pane draws its buttons.
    w.relayout(&Fixed);
    w.record_side(&Fixed);
    assert!(w
        .side
        .hits
        .iter()
        .any(|(_, h)| *h == selpane::SideHit::Close));
}

#[test]
fn insert_menus_and_collapsed_group_labels() {
    let mut w = window();
    // The Insert tab (its buttons anchor the menus).
    let [x, y, tw, th] = w.ribbon.rect_of(RibbonHit::Tab(1)).expect("tab");
    let (x, y) = (x + tw / 2.0, y + th / 2.0);
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
    assert!(w.action("sl.pictures"));
    assert_eq!(menu_labels(&w), ["This Device..."]);
    assert!(w.choose(Choice::Command("ins:picture".into())));
    assert_eq!(w.file_request, Some(InsertFile::Picture));
    assert!(w.action("sl.photoAlbum"));
    assert!(w.choose(Choice::Command("ins:album".into())));
    assert_eq!(w.file_request, Some(InsertFile::AlbumPicture));
    assert!(sr::has_menu_mark("sl.model3d") && !sr::has_menu_mark("sl.smartart"));
    // Links / Symbols / Media / Editing: no label under their one button.
    assert_eq!(w.strings.get(ins::COLLAPSED), "");
}

#[test]
fn preview_and_delete_comment_follow_the_slide() {
    let mut w = window();
    let f = ribbon_format(&w.view, false);
    assert!(!slide_enabled("transition.preview", &f));
    assert!(!slide_enabled("anim.preview", &f));
    assert!(!slide_enabled("review.deleteComment", &f));
    assert!(w.set_transition(Some("fade"), None, None, false));
    let f = ribbon_format(&w.view, false);
    assert!(slide_enabled("transition.preview", &f));
}

#[test]
fn large_buttons_dim_only_where_the_reference_does() {
    use crate::ribbon::Item;
    let w = window();
    let f = ribbon_format(&w.view, false);
    let mut tabs = w.fixed_tabs();
    for c in [
        ins::Context::Shape,
        ins::Context::Picture,
        ins::Context::Table,
    ] {
        tabs.extend(ins::context_tabs(c));
    }
    let mut dim: Vec<&str> = tabs
        .iter()
        .flat_map(|t| t.groups.iter())
        .flat_map(|g| g.large.iter())
        .filter_map(|i| match i {
            Item::Large(id) if !slide_enabled(id, &f) => Some(*id),
            _ => None,
        })
        .collect();
    dim.sort_unstable();
    dim.dedup();
    // Quick Styles is dimmed on the reference without a selection too; the others of
    // the list are on contextual tabs, shown only with a selection.
    assert_eq!(
        dim,
        [
            "anim.preview",
            "draw.inkHelp",
            "draw.inkToMath",
            "draw.replay",
            "review.deleteComment",
            "sl.altText",
            "sl.hideInk",
            "sl.quickStyles",
            "sl.textQuick",
            "sl.translate",
            "transition.preview"
        ],
        "large buttons dimmed with nothing selected"
    );
}

#[test]
fn insert_object_embeds_a_new_workbook() {
    let mut w = window();
    let n = w
        .view
        .engine()
        .presentation()
        .map_or(0, |p| p.slides[w.view.current()].data.shapes.len());
    assert!(w.choose(Choice::Command("ins:workbook".into())));
    let p = w.view.engine().presentation().expect("deck");
    assert_eq!(p.slides[w.view.current()].data.shapes.len(), n + 1);
    assert_eq!(w.view.selection().len(), 1);
    assert!(w.choose(Choice::Command("ins:object".into())));
    assert_eq!(w.file_request, Some(InsertFile::Object));
    // Screenshot is shown, greyed (the host has no window pictures for it yet).
    let f = ribbon_format(&w.view, false);
    assert!(!slide_enabled("sl.screenshot", &f));
    assert!(slide_enabled("sl.object", &f));
}

struct Nothing;
impl Painter for Nothing {
    fn clear(&mut self, _: bo_render::Rgba) {}
    fn fill_rect(&mut self, _: f32, _: f32, _: f32, _: f32, _: bo_render::Rgba) {}
    fn page(&mut self, _: usize, _: i32, _: i32) {}
    fn text(&mut self, _: &str, _: f32, _: f32, _: TextStyle, _: bo_render::Rgba) {}
    fn path(
        &mut self,
        _: &[bo_render::PathSeg],
        _: Option<bo_render::Rgba>,
        _: Option<(bo_render::Rgba, f32)>,
    ) {
    }
    fn push_clip(&mut self, _: f32, _: f32, _: f32, _: f32) {}
    fn pop_clip(&mut self) {}
}

#[test]
fn every_tab_paints() {
    let mut w = window();
    for i in 0..9 {
        let [x, y, tw, th] = w.ribbon.rect_of(RibbonHit::Tab(i)).expect("tab");
        let (x, y) = (x + tw / 2.0, y + th / 2.0);
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
        w.handle(Input::Move { x: 700.0, y: 900.0 }, &Fixed);
        w.paint(&mut Nothing);
    }
}

#[test]
fn review_groups_accessibility_language_and_smartart_column() {
    let mut w = window();
    // Paragraph: Text Direction, Align Text, Convert to SmartArt in one column of three.
    let a = w.command_rect("sl.textDir").expect("Text Direction");
    let b = w.command_rect("sl.alignText").expect("Align Text");
    let c = w
        .command_rect("sl.toSmartArt")
        .expect("Convert to SmartArt");
    assert!((a[0] - c[0]).abs() < 1.0 && (b[0] - c[0]).abs() < 1.0);
    assert!(a[1] < b[1] && b[1] < c[1], "{a:?} {b:?} {c:?}");
    // Convert to SmartArt: the text box becomes a diagram with one node per paragraph.
    let id = text_box(&mut w, "One\nTwo\nThree");
    assert!(w.choose(Choice::Command("tosmart:process".into())));
    let p = w.view.engine().presentation().expect("deck");
    let shapes = bo_engine::slide_edit::a11y::slide_shapes(p, w.view.current());
    assert!(
        !shapes
            .iter()
            .any(|s| s.kind == "shape" && s.text.as_deref().is_some_and(|t| t.contains("Two"))),
        "text box {id} replaced"
    );
    assert!(shapes.iter().any(|s| s.kind == "smartart"));
    // Accessibility: the new diagram lacks alternative text.
    assert!(w.choose(Choice::Command("rv:check".into())));
    assert_eq!(w.side.kind, selpane::SideKind::Accessibility);
    let rows = w.a11y_rows();
    assert!(
        rows.iter().any(|r| r.kind == "Missing alternative text"),
        "{rows:?}"
    );
    // Language: English, then unchecked.
    let id = text_box(&mut w, "Hello");
    assert!(w.choose(Choice::Command("rv:lang:en-US".into())));
    let lang = |w: &SlideWindow| {
        let p = w.view.engine().presentation().expect("deck");
        p.slides[w.view.current()]
            .data
            .shapes
            .iter()
            .find_map(|n| match n {
                bo_model::slide::ShapeNode::Shape(s) if s.nv.id == id => s.text.clone(),
                _ => None,
            })
            .and_then(|t| match &t.paragraphs[0].runs[0] {
                bo_model::slide::TextRun::Run { props, .. } => Some(props.clone()),
                _ => None,
            })
            .expect("run")
    };
    assert_eq!(lang(&w).lang.as_deref(), Some("en-US"));
    assert!(w.choose(Choice::Command("rv:noproof".into())));
    assert!(lang(&w).attrs.iter().any(|(k, _)| k == "noProof"));
    assert_eq!(
        spelling::access("Change All", 'l'),
        "Change A&ll".replace("A&ll", "Al&l")
    );
}
