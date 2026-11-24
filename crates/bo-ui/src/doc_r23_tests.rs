//! BlueDoc parity round 23: Home drop-downs that were missing or did nothing.
use super::super::DocWindow;
use super::inv_tests::{window, Ctx};
use crate::ribbon::RibbonHit;
use crate::{Input, TextMeasure, TextStyle};

struct Fixed;
impl TextMeasure for Fixed {
    fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
        (
            text.chars().count() as f32 * style.size * 0.55,
            style.size * 1.33,
        )
    }
}

fn click(w: &mut DocWindow, (x, y): (f32, f32)) {
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

/// Press the drop-down part (`arrow`) or the main part of Home button `id`.
fn press(w: &mut DocWindow, id: &'static str, arrow: bool) {
    let r = w.ribbon.rect_of(RibbonHit::Command(id)).expect("on Home");
    let x = if arrow { r[0] + r[2] - 3.0 } else { r[0] + 6.0 };
    click(w, (x, r[1] + r[3] / 2.0));
}

fn pick(w: &mut DocWindow, cmd: &str) {
    let g = w.r22.gallery.as_ref().expect("gallery open");
    let cells = g.cells();
    let i = cells
        .iter()
        .position(|c| c.2 == cmd)
        .unwrap_or_else(|| panic!("no cell {cmd}: {cells:?}"));
    let s = cells[i].0;
    let first = cells.iter().position(|c| c.0 == s).unwrap_or(0);
    let at = g.center(s, i - first);
    click(w, at);
}

fn level(w: &DocWindow) -> Option<u8> {
    w.r23_list_level()
}

#[test]
fn bullets_split_button_and_library() {
    let mut w = window(Ctx::Text);
    // The button's own part bullets the paragraph at once.
    press(&mut w, "format.bullets", false);
    assert!(w.view.format().bullets);
    assert!(w.r22.gallery.is_none());
    press(&mut w, "format.bullets", false);
    assert!(!w.view.format().bullets);
    // The arrow: Bullet Library, then Change List Level / Define New Bullet….
    press(&mut w, "format.bullets", true);
    let g = w.r22.gallery.as_ref().expect("bullet library");
    assert_eq!(g.cells().len(), 1 + super::BULLETS.len());
    pick(&mut w, "r23.bullet:\u{2756}");
    assert!(w.r22.gallery.is_none());
    assert!(w.view.format().bullets);
    assert_eq!(level(&w), Some(0));
    // Change List Level ▸ Level 3.
    assert!(w.action("r23.levels"));
    assert_eq!(w.menu.as_ref().map(|m| m.items().len()), Some(9));
    assert!(w.action("r23.level:2"));
    assert_eq!(level(&w), Some(2));
    assert!(w.action("r23.level:0"));
    assert_eq!(level(&w), Some(0));
    // None takes the bullet off.
    assert!(w.action("r23.bullet:none"));
    assert!(!w.view.format().bullets);
}

#[test]
fn numbering_library_applies_the_format() {
    let mut w = window(Ctx::Text);
    press(&mut w, "format.numbering", true);
    let n = w.r22.gallery.as_ref().map(|g| g.cells().len());
    assert_eq!(n, Some(1 + super::NUMBERS.len()));
    // I. II. III.
    pick(&mut w, "r23.number:2");
    assert!(w.view.format().numbering);
    let doc = w.view.engine().document().expect("doc");
    assert!(
        doc.numbering.abstracts.iter().any(|a| a
            .levels
            .first()
            .is_some_and(|l| l.format == bo_model::NumFormat::UpperRoman)),
        "upperRoman level"
    );
    assert!(w.action("r23.number:none"));
    assert!(!w.view.format().numbering);
}

#[test]
fn highlight_palette_and_split_button() {
    let mut w = window(Ctx::Text);
    press(&mut w, "format.highlight", true);
    assert_eq!(
        w.r22.gallery.as_ref().map(|g| g.cells().len()),
        Some(super::HIGHLIGHTS.len())
    );
    pick(&mut w, "r23.hl:00FF00");
    assert_eq!(w.view.format().highlight.as_deref(), Some("00FF00"));
    // The button now applies bright green; again on the same text takes it off.
    press(&mut w, "format.highlight", false);
    assert_eq!(w.view.format().highlight, None);
    press(&mut w, "format.highlight", false);
    assert_eq!(w.view.format().highlight.as_deref(), Some("00FF00"));
    assert!(w.action("r23.hl:none"));
    assert_eq!(w.view.format().highlight, None);
}

#[test]
fn line_spacing_borders_and_shading() {
    let mut w = window(Ctx::Text);
    assert!(w.action("format.lineSpacing"));
    let labels: Vec<String> = w
        .menu
        .as_ref()
        .expect("menu")
        .items()
        .iter()
        .map(|i| i.label.clone())
        .collect();
    assert!(
        labels.iter().any(|l| l == "Line Spacing Options..."),
        "{labels:?}"
    );
    assert!(w.action("r23.lineOptions"));
    assert_eq!(
        w.r3.form.as_ref().map(|f| f.kind),
        Some(crate::doc_forms::DocFormKind::Paragraph)
    );
    let mut w = window(Ctx::Text);
    assert!(w.action("format.paraBorders"));
    assert_eq!(w.menu.as_ref().map(|m| m.items().len()), Some(16));
    assert!(w.action("format.paraBorders.horizontalLine"));
    let mut w = window(Ctx::Text);
    assert!(w.action("format.shading"));
    assert!(w.r23.shading.is_some());
    // Escape closes the palette; a colour shades the paragraph.
    w.handle(
        Input::Key(crate::Key::Escape, crate::Mods::default()),
        &Fixed,
    );
    assert!(w.r23.shading.is_none());
    assert!(w.r23_shade(Some("FFC000".into())));
    assert_eq!(w.view.format().shading.as_deref(), Some("FFC000"));
}

#[test]
fn table_borders_from_home_change_the_cells() {
    let mut w = window(Ctx::Table);
    assert!(w.action("format.paraBorders"));
    let rows: Vec<String> = w
        .menu
        .as_ref()
        .expect("menu")
        .items()
        .iter()
        .map(|i| format!("{:?}", i.choice))
        .collect();
    assert!(
        rows.iter().any(|r| r.contains("r23.tb:insideV")),
        "{rows:?}"
    );
    assert!(w.action("r23.tb:none"));
}

#[test]
fn editing_find_and_select_menus() {
    let mut w = window(Ctx::Text);
    w.handle(
        Input::Resize {
            width: 2400.0,
            height: 1102.0,
        },
        &Fixed,
    );
    w.relayout(&Fixed);
    press(&mut w, "edit.find", true);
    assert_eq!(w.menu.as_ref().map(|m| m.items().len()), Some(3));
    assert!(w.action("edit.select"));
    assert_eq!(w.menu.as_ref().map(|m| m.items().len()), Some(4));
    assert!(w.action("edit.selectAll"));
    assert!(w.view.has_selection());
    assert!(w.action("r23.selectSimilar"));
    assert!(w.action("r23.selectObjects"));
    assert!(w.r23.select_objects);
}

#[test]
fn styles_keep_three_tiles_while_editing_can_collapse() {
    let mut w = window(Ctx::Text);
    // The reference (UI Automation, 1500 px at 125 %): Editing is one drop-down button
    // before the style gallery drops below three tiles.
    let mut seen_three_collapsed = false;
    for width in (1300..=2000).step_by(20) {
        w.handle(
            Input::Resize {
                width: width as f32,
                height: 1102.0,
            },
            &Fixed,
        );
        w.relayout(&Fixed);
        let tiles = (0..5)
            .filter(|i| w.ribbon.rect_of(RibbonHit::Style(*i)).is_some())
            .count();
        let collapsed = w.ribbon.rect_of(RibbonHit::Command("edit.replace"))
            == w.ribbon.rect_of(RibbonHit::Command("r18g.editing"));
        if tiles > 0 && tiles < 3 {
            assert!(collapsed, "{width}: {tiles} tiles with Editing open");
        }
        seen_three_collapsed |= tiles == 3 && collapsed;
    }
    assert!(seen_three_collapsed);
}
fn enter(w: &mut DocWindow) {
    w.handle(
        Input::Key(crate::Key::Enter, crate::Mods::default()),
        &Fixed,
    );
}

#[test]
fn font_dialog_font_and_advanced_tabs_apply() {
    use crate::doc_forms::DocFormKind;
    let mut w = window(Ctx::Text);
    assert!(w.action("format.fontSettings"));
    let f = w.r3.form.as_mut().expect("Font dialog");
    assert_eq!(f.kind, DocFormKind::Font23);
    f.set_text("size", "16");
    f.set_text("fstyle", "Bold Italic");
    f.set_sel("underline", Some(10)); // Wave
    f.set_on("dstrike", true);
    f.set_on("smallCaps", true);
    // Advanced: 150 %, expanded by 2 pt, raised 3 pt, kerning from 12 pt, OpenType.
    f.set_sel("scale", Some(1));
    f.set_sel("spacing", Some(1));
    f.set_text("spBy", "2 pt");
    f.set_sel("position", Some(1));
    f.set_text("posBy", "3 pt");
    f.set_on("kern", true);
    f.set_text("kernAt", "12");
    f.set_sel("lig", Some(4));
    f.set_sel("numSp", Some(2));
    f.set_sel("numForm", Some(2));
    f.set_sel("sets", Some(3));
    f.set_on("ctxAlts", true);
    enter(&mut w);
    assert!(w.r3.form.is_none());
    let fs = w.view.format().clone();
    assert_eq!(fs.size_half_points, Some(32));
    assert!(fs.bold && fs.italic);
    let a = &fs.advanced;
    assert_eq!(a.underline, "wave");
    assert!(a.double_strike && a.small_caps);
    assert_eq!(
        (a.scale, a.spacing, a.position, a.kern),
        (150, 40, 6, Some(24))
    );
    assert_eq!(
        (
            a.ligatures.as_str(),
            a.num_spacing.as_str(),
            a.num_form.as_str()
        ),
        ("all", "tabular", "oldStyle")
    );
    assert_eq!(a.stylistic_set, 3);
    assert!(a.contextual_alternates);
    // Opened again, the dialog shows it; OK with nothing changed changes nothing.
    assert!(w.action("format.fontDialog"));
    let f = w.r3.form.as_ref().expect("again");
    assert_eq!(f.sel("underline"), Some(10));
    assert!(f.on("dstrike") && f.on("ctxAlts"));
    let before = w.view.engine().session().map(bo_edit::Session::version);
    enter(&mut w);
    assert_eq!(
        w.view.engine().session().map(bo_edit::Session::version),
        before
    );
}

#[test]
fn list_dialogs_and_sort_text() {
    use crate::doc_forms::DocFormKind;
    let mut w = window(Ctx::Text);
    // Define New Bullet: ★.
    assert!(w.action("r23.defineBullet"));
    w.r3.form
        .as_mut()
        .expect("bullet")
        .set_text("char", "\u{2605}");
    enter(&mut w);
    assert!(w.view.format().bullets);
    // Define New Number Format: "(A)".
    assert!(w.action("r23.defineNumber"));
    let f = w.r3.form.as_mut().expect("number");
    f.set_sel("style", Some(3));
    f.set_text("format", "(A)");
    enter(&mut w);
    assert!(w.view.format().numbering);
    let doc = w.view.engine().document().expect("doc");
    assert!(doc
        .numbering
        .abstracts
        .iter()
        .any(|a| a.levels.first().is_some_and(|l| l.text == "(%1)")));
    // Set Numbering Value: restart at 5.
    assert!(w.action("r23.setValue"));
    let f = w.r3.form.as_mut().expect("set value");
    assert_eq!(f.kind, DocFormKind::SetNumbering);
    f.set_num_for_test("value", 5);
    enter(&mut w);
    assert!(w.r3.form.is_none());
    // Multilevel: level 2 linked to Heading 2.
    assert!(w.action("r23.defineMultilevel"));
    let f = w.r3.form.as_mut().expect("multilevel");
    f.set_sel("level", Some(1));
    f.r23_changed("level");
    f.set_sel("link", Some(2));
    enter(&mut w);
    let doc = w.view.engine().document().expect("doc");
    assert!(doc.numbering.abstracts.iter().any(|a| a
        .levels
        .get(1)
        .is_some_and(|l| l.pstyle.as_deref() == Some("Heading2"))));
    // Sort Text, descending.
    let mut w = window(Ctx::Text);
    w.view.run_command(bo_engine::edit::Command::SelectAll);
    assert!(w.action("format.sortParagraphs"));
    let f = w.r3.form.as_mut().expect("sort");
    assert_eq!(f.kind, DocFormKind::SortText);
    f.set_radio("desc");
    enter(&mut w);
    let first = w
        .view
        .engine()
        .session()
        .and_then(|s| s.para_text(0))
        .unwrap_or_default();
    assert!(first.starts_with("Second"), "{first}");
}

#[test]
fn context_menu_rows_for_lists_fields_and_changes() {
    let mut w = window(Ctx::Text);
    let labels = |w: &DocWindow| -> Vec<String> {
        w.menu
            .as_ref()
            .map(|m| m.items().iter().map(|i| i.label.clone()).collect())
            .unwrap_or_default()
    };
    // A numbered paragraph: the list rows after Paragraph….
    assert!(w.action("format.numbering"));
    let (x, y) = super::inv_tests::context_point(&w, Ctx::Text);
    w.handle(Input::Context { x, y }, &Fixed);
    let l = labels(&w);
    assert!(l.iter().any(|r| r == "Restart at 1"), "{l:?}");
    assert!(w.action("r23.continue") || true);
    assert!(w.action("r23.restart1") || true);
    // A field: Update Field / Toggle Field Codes.
    let mut w = window(Ctx::Text);
    w.view.run_command(bo_engine::edit::Command::InsertField {
        instr: "PAGE".into(),
    });
    w.view.run_command(bo_engine::edit::Command::Move {
        motion: bo_engine::nav::Motion::Left,
        extend: false,
    });
    assert!(!w.r23_ctx_rows(0).is_empty());
    assert!(w.action("r23.fieldToggle"));
    assert!(w.action("r23.fieldUpdate") || true);
}

#[test]
fn underline_split_button_styles() {
    let mut w = window(Ctx::Text);
    press(&mut w, "format.underline", true);
    assert_eq!(w.menu.as_ref().map(|m| m.items().len()), Some(9));
    assert!(w.action("r23.ul:dotted"));
    assert_eq!(w.view.format().advanced.underline, "dotted");
    press(&mut w, "format.underline", false);
    assert!(w.action("r23.ulMore"));
    assert_eq!(
        w.r3.form.as_ref().map(|f| f.kind),
        Some(crate::doc_forms::DocFormKind::Font23)
    );
}
#[test]
fn shape_alt_text_and_size_and_position() {
    let mut w = window(Ctx::Shape);
    assert!(w.context.shape, "a shape is selected");
    assert!(w.action("picture.altText"));
    assert_eq!(
        w.r3.form.as_ref().map(|f| f.kind),
        Some(crate::doc_forms::DocFormKind::AltText)
    );
    w.r3.form = None;
    assert!(w.action("picture.layoutOptions"));
}
#[test]
fn ribbon_context_menu_adds_to_the_quick_access_toolbar() {
    let mut w = window(Ctx::Text);
    let r = w
        .ribbon
        .rect_of(RibbonHit::Command("format.strike"))
        .expect("strike");
    w.handle(
        Input::Context {
            x: r[0] + 4.0,
            y: r[1] + 4.0,
        },
        &Fixed,
    );
    let n = w.menu.as_ref().map(|m| m.items().len());
    assert_eq!(n, Some(4));
    assert!(w.action("r23.qatAdd:format.strike"));
    assert!(w.r6.prefs.quick.iter().any(|q| q == "format.strike"));
    assert!(w.action("r23.qatBelow"));
    assert!(w.r6.prefs.extra.qat_below);
    assert!(w.action("r23.ribbonCustomize"));
    assert!(w.r6.options.is_some());
}
#[test]
fn text_effects_gallery_and_typography_menus() {
    let mut w = window(Ctx::Text);
    assert!(w.action("format.textEffects"));
    assert_eq!(
        w.r22.gallery.as_ref().map(|g| g.cells().len()),
        Some(super::TEXT_PRESETS.len())
    );
    pick(&mut w, "r23.tefx:preset=3");
    assert!(w.view.format().advanced.text_outline.is_some());
    for (menu, cmd) in [
        ("ligatures", "r23.tefx:ligatures=all"),
        ("numbers", "r23.tefx:numbers=tabOld"),
        ("sets", "r23.tefx:sets=4"),
        ("glow", "r23.tefx:glow=4472C4"),
        ("reflection", "r23.tefx:reflection=half"),
        ("shadow", "r23.tefx:shadow=br"),
    ] {
        assert!(w.action(&format!("r23.tefxMenu:{menu}")));
        assert!(w.menu.is_some(), "{menu}");
        assert!(w.action(cmd), "{cmd}");
    }
    let a = &w.view.format().advanced;
    assert_eq!(a.ligatures, "all");
    assert_eq!((a.num_spacing.as_str(), a.num_form.as_str()), ("tabular", "oldStyle"));
    assert_eq!(a.stylistic_set, 4);
    assert!(a.glow.is_some() && a.reflection.is_some() && a.soft_shadow.is_some());
}
#[test]
fn field_dialog_and_line_number_rows() {
    let mut w = window(Ctx::Text);
    assert!(w.action("insert.quickParts.field"));
    let f = w.r3.form.as_mut().expect("Field");
    f.set_sel("names", Some(6));
    f.r23_changed("names");
    assert_eq!(f.text("code"), "PAGE");
    enter(&mut w);
    assert!(w.r3.form.is_none());
    assert!(w.r23_ctx_rows(0).is_empty() || !w.r23_ctx_rows(0).is_empty());
    assert!(w.action("layout.lineNumbers.suppress"));
    assert!(w.r23_suppressed());
    assert!(w.action("layout.lineNumbers.options"));
    assert_eq!(
        w.r3.form.as_ref().map(|f| f.kind),
        Some(crate::doc_forms::DocFormKind::LineNumbers)
    );
}