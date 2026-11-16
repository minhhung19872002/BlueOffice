//! Tests of BlueSlide round 7: font size steps, the Line Spacing menu and Paragraph dialog,
//! the Format Painter on text (headless).
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

/// A text box reading `text`, selected as a shape; its id.
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

fn select_text(w: &mut SlideWindow, id: u32, start: usize, end: usize) {
    assert!(w.view.run(SlideCommand::TextSelect {
        id: Some(id),
        start,
        end,
        cell: None,
    }));
}

fn ctrl(c: char, shift: bool) -> Input {
    Input::Key(
        Key::Letter(c),
        Mods {
            ctrl: true,
            shift,
            ..Mods::default()
        },
    )
}

#[test]
fn clicking_tabs_one_after_another_switches_each_time() {
    let mut w = window();
    let mut tab_x = Vec::new();
    for x in (0..1400).step_by(4) {
        if let Some(RibbonHit::Tab(i)) = w.ribbon.hit(x as f32, w.ribbon.top() + 10.0) {
            if !tab_x.iter().any(|(j, _)| *j == i) {
                tab_x.push((i, x as f32 + 8.0));
            }
        }
    }
    assert!(tab_x.len() > 4, "{tab_x:?}");
    let y = w.ribbon.top() + 10.0;
    for &(i, x) in tab_x.iter().take(6) {
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
        // The host's timers run between clicks (tooltips, spelling, blinking).
        for _ in 0..20 {
            w.handle(Input::Tick, &Fixed);
            w.handle(Input::Blink, &Fixed);
        }
        w.handle(Input::Leave, &Fixed);
        assert_eq!(w.ribbon.active_tab(), i, "tab {i} at {x}");
    }
}

#[test]
fn font_size_keys_step_through_the_reference_sizes() {
    let mut w = window();
    let id = text_box(&mut w, "Alpha");
    select_text(&mut w, id, 0, 5);
    w.view.format_text(TextFormat::Size { points: 18.0 });
    let size = |w: &SlideWindow| w.view.format().size;
    w.handle(ctrl(']', false), &Fixed);
    assert_eq!(size(&w), Some(20.0));
    w.handle(ctrl('.', true), &Fixed);
    assert_eq!(size(&w), Some(24.0));
    w.handle(ctrl('[', false), &Fixed);
    w.handle(ctrl(',', true), &Fixed);
    assert_eq!(size(&w), Some(18.0));
    w.handle(ctrl('[', false), &Fixed);
    assert_eq!(size(&w), Some(16.0));
    // The ribbon buttons step the same way.
    assert!(w.action("format.growStep"));
    assert_eq!(size(&w), Some(18.0));
}

#[test]
fn line_spacing_menu_and_paragraph_dialog() {
    let mut w = window();
    let id = text_box(&mut w, "One\nTwo");
    select_text(&mut w, id, 0, 7);
    assert!(w.action("format.lineSpacing"));
    let labels: Vec<String> = w
        .menu
        .as_ref()
        .map(|m| m.items().iter().map(|i| i.label.clone()).collect())
        .expect("menu");
    assert_eq!(labels.len(), 6, "{labels:?}");
    assert_eq!(labels[1], "1.5");
    assert!(w.choose(Choice::Command("lspc:1.5".into())));
    assert_eq!(w.view.format().line_spacing, Some(1.5));
    // Line Spacing Options: the Paragraph dialog, its values applied with OK.
    assert!(w.action("format.paragraphSettings"));
    let mut v = w.dialog.as_ref().map(|(d, _)| d.values()).expect("dialog");
    assert_eq!(v.len(), 5);
    assert_eq!(v[4], "1.5");
    v[0] = "0.5\"".into();
    v[2] = "12 pt".into();
    v[3] = "6 pt".into();
    v[4] = "0.9".into();
    w.dialog_button(
        ins::SlideDialog::Paragraph,
        crate::dialog::DialogButton::Ok,
        &v,
    );
    assert!(w.dialog.is_none());
    let f = w.view.format();
    let p = f.paragraph.expect("paragraph values");
    assert!((p.before_text - 36.0).abs() < 0.01, "{p:?}");
    assert_eq!((p.space_before, p.space_after), (Some(12.0), Some(6.0)));
    assert_eq!(f.line_spacing, Some(0.9));
    // Exact spacing in points.
    assert!(w.action("format.paragraphSettings"));
    let mut v = w.dialog.as_ref().map(|(d, _)| d.values()).expect("dialog");
    v[4] = "24 pt".into();
    w.dialog_button(
        ins::SlideDialog::Paragraph,
        crate::dialog::DialogButton::Ok,
        &v,
    );
    assert_eq!(
        w.view.format().paragraph.and_then(|p| p.line_exact),
        Some(24.0)
    );
}

#[test]
fn change_case_and_clear_formatting() {
    assert_eq!(
        paragraph::change_case("hello world. again", "sentence"),
        "Hello world. Again"
    );
    assert_eq!(
        paragraph::change_case("hello big world", "title"),
        "Hello Big World"
    );
    assert_eq!(paragraph::change_case("AbC", "toggle"), "aBc");
    let mut w = window();
    let id = text_box(&mut w, "alpha beta");
    select_text(&mut w, id, 0, 5);
    assert!(w.choose(Choice::Command("case:upper".into())));
    let te = w.view.text_edit().cloned().unwrap();
    assert_eq!(te.text, "ALPHA beta");
    assert_eq!((te.start, te.end), (0, 5));
    w.handle(ctrl('b', false), &Fixed);
    assert!(w.view.format().bold);
    assert!(w.action("sl.clearFmt"));
    assert!(!w.view.format().bold);
    // Character Spacing: Loose = +3 pt.
    assert!(w.choose(Choice::Command("chsp:3".into())));
    assert_eq!(w.view.format().spacing, Some(3.0));
    // Text Shadow toggles, shown as on.
    assert!(w.action("sl.shadow"));
    assert!(w.view.format().shadow);
    assert!(w.toggled().contains(&"sl.shadow"));
    assert!(w.action("sl.shadow"));
    assert!(!w.view.format().shadow);
    // Highlight and font colour menus.
    assert!(w.action("sl.highlight"));
    assert!(w.menu.is_some());
    assert!(w.choose(Choice::Command("hl:00FF00".into())));
    assert_eq!(w.view.format().highlight.as_deref(), Some("00FF00"));
    assert!(w.choose(Choice::Command("fc:FF0000".into())));
    assert_eq!(w.view.format().color.as_deref(), Some("FF0000"));
}

#[test]
fn text_box_options_of_the_paragraph_group() {
    let mut w = window();
    let id = text_box(&mut w, "One two three");
    // Text Direction / Align Text sit in a column right of the two rows.
    let a = w.command_rect("sl.textDir").expect("Text Direction");
    let b = w.command_rect("sl.alignText").expect("Align Text");
    let j = w.command_rect("format.justify").expect("Justify");
    assert!(
        (a[0] - b[0]).abs() < 1.0 && a[0] > j[0],
        "{a:?} {b:?} {j:?}"
    );
    for c in ["body:anchor:b", "body:cols:2", "body:vert:vert270"] {
        assert!(w.choose(Choice::Command(c.into())), "{c}");
    }
    let p = w.view.engine().presentation().unwrap();
    let bp = p.slides[w.view.current()]
        .data
        .shapes
        .iter()
        .find_map(|n| match n {
            bo_model::slide::ShapeNode::Shape(s) if s.nv.id == id => {
                s.text.as_ref().map(|t| t.body_pr.clone())
            }
            _ => None,
        })
        .unwrap();
    assert_eq!(bp.anchor.as_deref(), Some("b"));
    assert_eq!(bp.num_col, Some(2));
    assert_eq!(bp.vert.as_deref(), Some("vert270"));
}

#[test]
fn insert_chart_lists_types_and_shows_the_chart_tabs() {
    let mut w = window();
    assert!(w.action("sl.chart"));
    assert!(w.menu.is_some(), "chart types");
    assert!(w.choose(Choice::Command("slchart:line".into())));
    w.handle(Input::Tick, &Fixed);
    assert_eq!(w.context, ins::Context::Chart);
    let kind = |w: &SlideWindow| {
        w.view
            .state()
            .and_then(|s| s.chart.as_ref())
            .map(|c| c.kind.clone())
    };
    assert_eq!(kind(&w).as_deref(), Some("line"));
    // Change Chart Type and a style through the shared menus.
    assert!(w.action("sheet.chartChangeType"));
    assert!(w.menu.is_some());
    assert!(w.action("chartType.bar"));
    assert_eq!(kind(&w).as_deref(), Some("bar"));
    let style = format!("chartStyle.{}", bo_sheet::chart_style::STYLES[2]);
    assert!(w.action(&style));
    assert!(w.action("chartEl.legendBottom"));
    assert_eq!(
        w.view
            .state()
            .and_then(|s| s.chart.as_ref())
            .and_then(|c| c.legend.clone())
            .as_deref(),
        Some("b")
    );
}

#[test]
fn format_painter_copies_character_formatting_to_selected_text() {
    let mut w = window();
    let id = text_box(&mut w, "Alpha Beta");
    select_text(&mut w, id, 0, 5);
    w.handle(ctrl('b', false), &Fixed);
    assert!(w.view.format().bold);
    assert!(w.action("sl.painter"));
    // Selecting other text and letting the mouse go pastes the formatting there.
    select_text(&mut w, id, 6, 10);
    assert!(!w.view.format().bold);
    let c = w.view.canvas_rect();
    assert!(w.paste_format_painter(Some((c[0] + c[2] / 2.0, c[1] + c[3] / 2.0))));
    select_text(&mut w, id, 6, 10);
    assert!(w.view.format().bold);
    assert!(w.fmt_painter.is_none(), "one use");
}
