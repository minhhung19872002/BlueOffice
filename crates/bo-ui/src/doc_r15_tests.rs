//! BlueDoc parity round 15: the Colors dialog (More Colors…) from Page Color and from a
//! dialog's colour drop-down, and the recent colours it leaves.
use super::super::DocWindow;
use crate::doc_forms::{DocFormKind, FormOut};
use crate::{DocView, Input, Key, Mods, TextMeasure, TextStyle};
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
    w
}

/// Click the centre of control `id` of the open dialog.
fn click(w: &mut DocWindow, id: &str) {
    let f = w.r3.form.as_mut().expect("form");
    f.layout(&Fixed, (1500.0, 1102.0), 1.25);
    let (x, y) = f.point_of(id).expect("control");
    let out = f.input(&Input::Press {
        x,
        y,
        count: 1,
        shift: false,
    });
    w.form_out(out);
}

fn enter(w: &mut DocWindow) {
    let out =
        w.r3.form
            .as_mut()
            .expect("form")
            .input(&Input::Key(Key::Enter, Mods::default()));
    assert!(matches!(out, FormOut::Run(..)), "{out:?}");
    w.form_out(out);
}

// source: the reference's Page Color › More Colors… (Colors dialog, captured round 15): the
// Standard tab's hexagon picks a colour shown as New; OK sets the page colour.
#[test]
fn more_colors_sets_the_page_colour_and_remembers_it() {
    let mut w = window();
    assert!(w.action("design.pageColor.more"));
    let f = w.r3.form.as_ref().expect("Colors");
    assert_eq!(f.kind, DocFormKind::Colors);
    assert!(f.r15_new_for_test().is_none());
    // The canvas's centre is a hexagon cell (row 9 of 13, sixth cell).
    click(&mut w, "hex6");
    assert_eq!(
        w.r3.form
            .as_ref()
            .and_then(crate::doc_forms::DocForm::r15_new_for_test),
        Some("FFCC99".to_owned())
    );
    enter(&mut w);
    assert!(w.r3.form.is_none());
    let bg = w
        .view
        .engine()
        .document()
        .and_then(|d| d.design.background.map(bo_model::Color::to_hex));
    assert_eq!(bg.as_deref(), Some("FFCC99"));
    assert_eq!(w.r15.recent, vec!["FFCC99".to_owned()]);
    // The Custom tab: the hex box sets the colour.
    assert!(w.action("design.pageColor.more"));
    let f = w.r3.form.as_mut().expect("Colors");
    f.set_tab(1);
    f.set_text("hex", "#1F4E79");
    f.r15_changed("hex");
    assert_eq!(f.r15_new_for_test(), Some("1F4E79".to_owned()));
    enter(&mut w);
    assert_eq!(w.r15.recent, vec!["1F4E79".to_owned(), "FFCC99".to_owned()]);
}

// source: the reference's Borders and Shading › Color ▾ › More Colors…: the colour comes
// back to that drop-down and the dialog stays open; a dialog kept under it survives.
#[test]
fn more_colors_of_a_drop_down_returns_to_its_dialog() {
    let mut w = window();
    assert!(w.r3_open_form(DocFormKind::TableOptions));
    w.form_out(FormOut::Stash(DocFormKind::BordersShading));
    assert!(w.r3.stash.is_some());
    let f = w.r3.form.as_mut().expect("Borders and Shading");
    f.r15_more_for_test("bColor");
    let out = f.click_for_test("moreColors");
    assert_eq!(out, FormOut::Stash(DocFormKind::Colors));
    w.form_out(out);
    assert_eq!(
        w.r3.form.as_ref().map(|f| f.kind),
        Some(DocFormKind::Colors)
    );
    let f = w.r3.form.as_mut().expect("Colors");
    f.set_tab(1);
    f.set_text("hex", "#C55A11");
    f.r15_changed("hex");
    enter(&mut w);
    let f = w.r3.form.as_ref().expect("back to Borders and Shading");
    assert_eq!(f.kind, DocFormKind::BordersShading);
    assert_eq!(f.r15_color_for_test("bColor"), "C55A11");
    assert_eq!(
        w.r3.stash.as_ref().map(|f| f.kind),
        Some(DocFormKind::TableOptions),
        "the dialog under Borders and Shading is kept"
    );
}

// source: the reference's Colors › Custom tab: the colour model switches the boxes to hue,
// saturation and luminance.
#[test]
fn custom_tab_switches_to_hsl() {
    let mut w = window();
    assert!(w.action("design.pageColor.more"));
    let f = w.r3.form.as_mut().expect("Colors");
    f.set_tab(1);
    f.set_text("hex", "#FF0000");
    f.r15_changed("hex");
    f.set_sel("model", Some(1));
    f.r15_changed("model");
    assert_eq!(f.label_for_test("c1L"), "Hue:");
    assert_eq!(f.text("c2"), "255");
    assert_eq!(f.text("c3"), "128");
}

/// Paragraphs "Intro" (heading), "a", "b", "Method" (heading), "c"; `collapsed`: Method has
/// Collapsed by default.
fn outline_engine(collapsed: bool) -> Engine {
    use bo_engine::edit::Command;
    let mut e = Engine::with_bundled_fonts();
    e.new_document("o.docx").expect("new");
    for (i, t) in ["Intro", "a", "b", "Method", "c"].iter().enumerate() {
        if i > 0 {
            e.command(Command::SplitParagraph).expect("split");
        }
        e.command(Command::InsertText { text: (*t).into() })
            .expect("text");
        // Split paragraphs take the properties before them: set each one's own.
        let heading = i == 0 || i == 3;
        let props = serde_json::json!({
            "outlineLevel": if heading { 1 } else { 10 },
            "collapsed": i == 3 && collapsed,
        });
        let cmd = serde_json::json!({ "cmd": "formatParagraph",
            "format": { "kind": "props", "format": props } });
        e.command_json(&cmd.to_string()).expect("props");
    }
    // Opened documents start with the caret at the top.
    e.command(Command::SetSelection {
        selection: bo_edit::Selection::caret(bo_edit::Pos::new(0, 0)),
    })
    .expect("caret");
    e
}

fn laid_out_ids(w: &DocWindow) -> Vec<u32> {
    let mut v: Vec<u32> = w
        .view
        .engine()
        .layout()
        .expect("layout")
        .pages
        .iter()
        .flat_map(|p| &p.lines)
        .filter_map(|l| match l.source {
            bo_layout::LineSource::Body(id) => Some(id),
            _ => None,
        })
        .collect();
    v.dedup();
    v
}

fn outline_window(collapsed: bool) -> DocWindow {
    let mut w = DocWindow::new(
        DocView::new(outline_engine(collapsed)).expect("view"),
        "en-US",
        "o.docx",
    );
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1500.0,
            height: 1102.0,
        },
        &Fixed,
    );
    w.r15_layout();
    w
}

// source: the reference's heading collapse (Expand / Collapse, captured round 15 with
// Paragraph.CollapsedState): the content under a heading up to the next heading of the same
// level disappears from the page; the document is not changed.
#[test]
fn collapsing_a_heading_hides_its_content_until_the_next_heading() {
    let mut w = outline_window(false);
    assert_eq!(laid_out_ids(&w), vec![0, 1, 2, 3, 4]);
    let saved = w.view.engine().document().cloned();
    assert!(w.r15_toggle(0));
    w.r15_layout();
    assert_eq!(laid_out_ids(&w), vec![0, 3, 4]);
    assert_eq!(
        w.view.engine().document().cloned(),
        saved,
        "view state only"
    );
    // Printing / exporting shows everything; the next layout pass hides it again.
    w.view.update_print_fields();
    assert_eq!(laid_out_ids(&w), vec![0, 1, 2, 3, 4]);
    w.r15_layout();
    assert_eq!(laid_out_ids(&w), vec![0, 3, 4]);
    // Enter at the end of the heading above: the collapsed heading's id follows.
    w.r15_toggle(0);
    w.r15_toggle(3);
    w.r15_layout();
    assert_eq!(laid_out_ids(&w), vec![0, 1, 2, 3]);
    w.view.run_command(bo_engine::edit::Command::SetSelection {
        selection: bo_edit::Selection::caret(bo_edit::Pos::new(0, 5)),
    });
    w.view.run_command(bo_engine::edit::Command::SplitParagraph);
    w.r15_layout();
    assert_eq!(w.r15.col.heads, vec![4]);
    assert_eq!(laid_out_ids(&w), vec![0, 1, 2, 3, 4]);
    // The caret moved into collapsed content expands its heading.
    w.view.run_command(bo_engine::edit::Command::SetSelection {
        selection: bo_edit::Selection::caret(bo_edit::Pos::new(5, 0)),
    });
    w.r15_layout();
    assert_eq!(w.r15.col.heads, Vec::<usize>::new());
    assert_eq!(laid_out_ids(&w), vec![0, 1, 2, 3, 4, 5]);
}

// [MS-DOCX] 2.6.1.1 w15:collapsed: a heading with Collapsed by default opens collapsed.
#[test]
fn collapsed_by_default_is_honoured_on_open() {
    let w = outline_window(true);
    assert_eq!(w.r15.col.heads, vec![3]);
    assert_eq!(laid_out_ids(&w), vec![0, 1, 2, 3]);
}

// The triangle shows on a hovered heading and a press on it collapses the heading.
#[test]
fn hovered_heading_triangle_collapses_it() {
    let mut w = outline_window(false);
    let starts = w.view.visible_para_starts();
    let (_, r) = starts
        .iter()
        .find(|(id, _)| *id == 0)
        .copied()
        .expect("Intro");
    assert!(w.r15_hover(r[0] + 5.0, r[1] + r[2] / 2.0));
    let (id, collapsed, tri) = w.r15.col.hover.expect("triangle");
    assert_eq!((id, collapsed), (0, false));
    assert!(tri[0] < r[0], "left of the heading text");
    // Body text has no triangle.
    let (_, rb) = starts.iter().find(|(id, _)| *id == 1).copied().expect("a");
    w.r15_hover(rb[0] + 5.0, rb[1] + rb[2] / 2.0);
    assert!(w.r15.col.hover.is_none());
    w.r15_hover(r[0] + 5.0, r[1] + r[2] / 2.0);
    let out = w.r15_input(&Input::Press {
        x: tri[0] + tri[2] / 2.0,
        y: tri[1] + tri[3] / 2.0,
        count: 1,
        shift: false,
    });
    assert_eq!(out, Some(true));
    w.r15_layout();
    assert_eq!(laid_out_ids(&w), vec![0, 3, 4]);
}

// source: the reference's Design › Paragraph Spacing (captured round 15): six presets read
// back black-box (Compact = 0 pt before, 4 pt after, single), Default restores the spacing
// the document opened with, Custom Paragraph Spacing… opens Manage Styles › Set Defaults.
#[test]
fn paragraph_spacing_presets_set_document_defaults() {
    let mut w = outline_window(false);
    let opened = w.r15.spacing_default.clone().expect("snapshot");
    assert!(w.action("design.paragraphSpacing"));
    let (x, y) = w.r15.spacing.as_ref().expect("menu").row_center(2);
    assert_eq!(
        w.r15_input(&Input::Press {
            x,
            y,
            count: 1,
            shift: false
        }),
        Some(true)
    );
    assert!(w.r15.spacing.is_none());
    let d = &w.view.engine().document().expect("doc").default_para;
    assert_eq!(d.space_after.map(|t| t.0), Some(80));
    assert_eq!(d.line_spacing, Some(bo_model::LineSpacing::Auto(240)));
    // The menu marks Compact as the spacing in use.
    assert!(w.action("design.paragraphSpacing"));
    assert_eq!(w.r15.spacing.as_ref().and_then(|m| m.current), Some(2));
    let (x, y) = w.r15.spacing.as_ref().expect("menu").row_center(0);
    w.r15_input(&Input::Press {
        x,
        y,
        count: 1,
        shift: false,
    });
    let d = w.view.engine().document().expect("doc");
    assert_eq!(super::default_spacing(d), opened);
    assert!(w.action("design.paragraphSpacing"));
    let (x, y) = w.r15.spacing.as_ref().expect("menu").row_center(7);
    w.r15_input(&Input::Press {
        x,
        y,
        count: 1,
        shift: false,
    });
    assert_eq!(
        w.r3.form.as_ref().map(|f| f.kind),
        Some(DocFormKind::ManageStyles)
    );
}

// source: the reference's Design › Colors › Customize Colors… (Create New Theme Colors,
// captured round 15): Save makes the twelve colours the theme's scheme (one undo step).
#[test]
fn customize_colors_saves_a_theme_colour_scheme() {
    let mut w = window();
    assert!(w.action("design.footer.colors.0"));
    let f = w.r3.form.as_mut().expect("Create New Theme Colors");
    assert_eq!(f.kind, DocFormKind::ThemeColors);
    assert_eq!(f.text("name"), "Custom 1");
    f.r15_set_color_for_test("tc4", "C00000");
    enter(&mut w);
    let d = w.view.engine().document().expect("doc");
    let accent1 = d
        .design
        .theme_colors
        .iter()
        .find(|(k, _)| k == "accent1")
        .map(|(_, c)| c.to_hex());
    assert_eq!(accent1.as_deref(), Some("C00000"));
    assert!(w.view.format().can_undo);
}

// source: the reference's Design › Fonts › Customize Fonts… (Create New Theme Fonts).
#[test]
fn customize_fonts_sets_the_theme_fonts() {
    let mut w = window();
    assert!(w.action("design.footer.fonts.0"));
    let f = w.r3.form.as_mut().expect("Create New Theme Fonts");
    assert_eq!(f.kind, DocFormKind::ThemeFonts);
    f.choose_item("major", "Cambria");
    enter(&mut w);
    let d = w.view.engine().document().expect("doc");
    assert_eq!(d.theme_fonts.major_latin, "Cambria");
}

// The Themes, Colors and Fonts drop-downs end in their command rows, as the reference's.
#[test]
fn design_drop_downs_have_command_rows() {
    use crate::design_gallery::DesignMenu;
    let w = window();
    assert_eq!(w.r15_design_footer(DesignMenu::Themes).len(), 3);
    assert_eq!(
        w.r15_design_footer(DesignMenu::Colors),
        vec!["Customize Colors..."]
    );
    assert_eq!(
        w.r15_design_footer(DesignMenu::Fonts),
        vec!["Customize Fonts..."]
    );
}
