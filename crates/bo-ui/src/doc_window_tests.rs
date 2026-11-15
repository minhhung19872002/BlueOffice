//! BlueDoc window: contextual tabs (table, header & footer, picture) and pictures from file.
use super::*;
use crate::{Key, Mods};
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

/// Press and release over the centre of ribbon button `id`.
fn click(w: &mut DocWindow, id: &'static str) {
    let [x, y, width, height] = w
        .ribbon
        .rect_of(RibbonHit::Command(id))
        .unwrap_or_else(|| panic!("{id} not on the ribbon"));
    let (x, y) = (x + width / 2.0, y + height / 2.0);
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

/// Run interface command `id` as the window does (then the contextual tabs follow).
fn act(w: &mut DocWindow, id: &str) {
    w.action(id);
    w.handle(Input::Tick, &Fixed);
}

/// Select the ribbon tab labelled `label`.
fn show_tab(w: &mut DocWindow, label: &str) {
    let i = w.ribbon.tab_index(label).expect("tab");
    let [x, y, tw, th] = w.ribbon.rect_of(RibbonHit::Tab(i)).expect("tab placed");
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
}

/// Choose entry `entry` of the open menu with the keyboard.
fn choose(w: &mut DocWindow, entry: usize) {
    for _ in 0..=entry {
        w.handle(Input::Key(Key::Down, Mods::default()), &Fixed);
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
}

#[test]
fn inserting_a_table_shows_its_design_and_layout_tabs() {
    let mut w = window();
    assert_eq!(w.ribbon.contextual_labels(), Vec::<&str>::new());
    act(&mut w, "insert.table.3x3");
    assert_eq!(
        w.ribbon.contextual_labels(),
        vec![doc_tools::TABLE_DESIGN, doc_tools::TABLE_LAYOUT]
    );
    // Inserting brings Table Design forward, as the reference does.
    assert_eq!(w.ribbon.active_label(), doc_tools::TABLE_DESIGN);
    // Layout tab: rows, columns, merge through the ribbon buttons.
    assert!(w.ribbon.tab_index(doc_tools::TABLE_LAYOUT) >= Some(w.ribbon.fixed_tab_count()));
    show_tab(&mut w, doc_tools::TABLE_LAYOUT);
    assert_eq!(w.ribbon.active_label(), doc_tools::TABLE_LAYOUT);
    click(&mut w, "table.insertRowBelow");
    click(&mut w, "table.insertColumnRight");
    let t = w.view.format().table.clone().expect("caret in the table");
    assert_eq!((t.rows, t.cols), (4, 4));
    // Merge is dimmed without a multi-cell selection.
    assert!(!crate::commands::enabled(
        "table.mergeCells",
        w.view.format()
    ));
    click(&mut w, "table.headerRow");
    assert!(w.view.format().table.as_ref().unwrap().header_row);
    assert!(w.on_ids().contains(&"table.headerRow"));
    // Delete menu: the whole table goes, and with it the tabs; Home comes back.
    click(&mut w, "table.delete");
    assert!(w.menu.is_some());
    choose(&mut w, 3);
    assert!(w.view.format().table.is_none());
    assert_eq!(w.ribbon.contextual_labels(), Vec::<&str>::new());
    assert_eq!(w.ribbon.active_label(), "ribbon.tabs.home");
    // Undo brings the table back; the tabs show without taking over.
    w.handle(
        Input::Key(
            Key::Letter('z'),
            Mods {
                ctrl: true,
                ..Mods::default()
            },
        ),
        &Fixed,
    );
    assert!(w.view.format().table.is_some());
    assert_eq!(w.ribbon.contextual_labels().len(), 2);
    assert_eq!(w.ribbon.active_label(), "ribbon.tabs.home");
}

#[test]
fn table_design_shading_and_borders_reach_the_saved_document() {
    let mut w = window();
    act(&mut w, "insert.table.2x2");
    assert_eq!(w.ribbon.active_label(), doc_tools::TABLE_DESIGN);
    click(&mut w, "table.shadingMenu");
    choose(&mut w, 1);
    assert_eq!(
        w.view.format().table.as_ref().unwrap().shading.as_deref(),
        Some("D9D9D9")
    );
    click(&mut w, "table.borders");
    choose(&mut w, 1);
    let bytes = w.view.save_docx().unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut doc = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("word/document.xml").unwrap(), &mut doc)
        .unwrap();
    assert!(doc.contains("w:fill=\"D9D9D9\""), "cell shading");
    assert!(doc.contains("w:tblBorders"), "table borders");
}

#[test]
fn header_and_footer_tab_while_editing_a_header() {
    let mut w = window();
    w.handle(Input::Text("Body".into()), &Fixed);
    act(&mut w, "insert.header");
    assert_eq!(w.view.story().map(|s| s.kind), Some("header"));
    assert_eq!(w.ribbon.active_label(), doc_tools::HEADER_FOOTER);
    w.handle(Input::Text("Head".into()), &Fixed);
    click(&mut w, "hf.differentFirst");
    assert_eq!(w.story_options().map(|s| s.0), Some(true));
    assert!(w.on_ids().contains(&"hf.differentFirst"));
    click(&mut w, "hf.gotoFooter");
    assert_eq!(w.view.story().map(|s| s.kind), Some("footer"));
    // Escape closes the footer and the tab; the tab used before comes back.
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(w.view.story().is_none());
    assert_eq!(w.ribbon.contextual_labels(), Vec::<&str>::new());
    assert_eq!(w.ribbon.active_label(), "ribbon.tabs.home");
    // Double click in the top margin opens the header again.
    w.view.scroll_to(0.0);
    let slot = w.view.slots()[0];
    let (x, y) = (
        (slot.x + slot.width / 2) as f32,
        (slot.y as f32 + 20.0 * w.view.scale()),
    );
    w.handle(
        Input::Press {
            x,
            y,
            count: 2,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Release { x, y }, &Fixed);
    assert_eq!(w.view.story().map(|s| s.kind), Some("header"));
    click(&mut w, "hf.close");
    assert!(w.view.story().is_none());
}

#[test]
fn insert_pictures_asks_the_host_and_shows_picture_format() {
    let mut w = window();
    show_tab(&mut w, "ribbon.tabs.insert");
    click(&mut w, "insert.picture");
    assert_eq!(w.take_requests(), vec![Request::PickPicture]);
    // A 1×1 PNG.
    let png = vec![
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 218, 99, 252, 207, 192, 240,
        31, 0, 5, 5, 2, 0, 95, 200, 241, 210, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    assert!(w.insert_picture(png, "dot.png"));
    assert!(w.refresh(&Fixed));
    let bytes = w.view.save_docx().unwrap();
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
    let mut doc = String::new();
    std::io::Read::read_to_string(&mut zip.by_name("word/document.xml").unwrap(), &mut doc)
        .unwrap();
    assert!(doc.contains("r:embed"), "picture in the document");
    // The new (inline) picture is selected: Picture Format comes forward.
    assert_eq!(w.ribbon.active_label(), doc_tools::PICTURE_FORMAT);
    click(&mut w, "picture.reset");
}

#[test]
fn every_contextual_button_has_a_label_an_icon_and_a_command() {
    let mut w = window();
    act(&mut w, "insert.table.3x3");
    act(&mut w, "insert.header");
    let story = w.story_options();
    assert!(story.is_some());
    let format = w.view.format().clone();
    let all = doc_tools::tabs(Context {
        story: Some(true),
        table: true,
        picture: true,
    });
    for tab in &all {
        assert_ne!(w.strings.get(tab.label), tab.label, "{}", tab.label);
        for g in &tab.groups {
            assert_ne!(w.strings.get(g.label), g.label, "{}", g.label);
            for item in g.large.iter().chain(g.rows.iter().flatten()) {
                let (crate::ribbon::Item::Large(id) | crate::ribbon::Item::Labeled(id)) = item
                else {
                    continue;
                };
                let key = format!("commands.{id}");
                assert_ne!(w.strings.get(&key), key, "label of {id}");
                assert!(w.icons.has(crate::ribbon::icon_of(id)), "icon of {id}");
                assert!(
                    catalog::is_menu(id)
                        || doc_tools::command(id, &format, story, 0).is_some()
                        || catalog::command(id, &format.section).is_some(),
                    "{id} does nothing"
                );
                for e in catalog::menu(id) {
                    assert!(
                        doc_tools::command(e.id, &format, story, 0).is_some()
                            || catalog::command(e.id, &format.section).is_some(),
                        "{} does nothing",
                        e.id
                    );
                }
            }
        }
    }
}

#[test]
fn navigation_pane_lists_headings_and_searches() {
    let mut w = window();
    w.handle(Input::Text("Intro".into()), &Fixed);
    w.view.run_command(Command::ApplyStyle {
        id: "Heading1".into(),
    });
    w.handle(Input::Key(Key::End, Mods::default()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    w.handle(Input::Text("body text here".into()), &Fixed);
    show_tab(&mut w, "ribbon.tabs.view");
    click(&mut w, "view.navigationPane");
    assert!(w.on_ids().contains(&"view.navigationPane"));
    let nav = w.nav.as_ref().expect("pane shown");
    assert_eq!(nav.headings.len(), 1);
    assert_eq!(nav.headings[0].text, "Intro");
    // The document area moved right of the pane.
    let pane_w = NavPane::width(w.scale);
    assert!(w.view.slots().iter().all(|s| s.x as f32 >= pane_w));
    // A heading row puts the caret at its paragraph.
    assert_eq!(w.view.caret_para(), Some(1));
    let (x, y) = (40.0, w.ribbon.bottom() + 142.0 + 10.0);
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
    assert_eq!(w.view.caret_para(), Some(0));
    // Ctrl+F puts the keyboard in the search box; Enter lists the matches.
    w.handle(
        Input::Key(
            Key::Letter('f'),
            Mods {
                ctrl: true,
                ..Mods::default()
            },
        ),
        &Fixed,
    );
    assert!(w.nav.as_ref().unwrap().focus);
    w.handle(Input::Text("text".into()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let nav = w.nav.as_ref().unwrap();
    assert_eq!(nav.tab, NavTab::Results);
    assert_eq!(nav.results.len(), 1);
    assert_eq!(nav.results[0].1, "text");
    assert!(w.view.has_selection(), "first match selected");
    // Closing the pane gives the width back.
    click(&mut w, "view.navigationPane");
    assert!(w.nav.is_none());
}

#[test]
fn misspelt_words_get_a_wavy_line_and_corrections_in_the_context_menu() {
    let mut w = window();
    w.handle(Input::Text("The houze is big".into()), &Fixed);
    for _ in 0..4 {
        w.handle(Input::Tick, &Fixed);
    }
    assert_eq!(w.view.proofing_issues(), 1, "{:?}", w.view.spell_marks());
    let m = w.view.spell_marks()[0].clone();
    let slot = w.view.slots()[m.page];
    let s = w.view.scale();
    let (x, y) = (
        slot.x as f32 + (m.x + m.w / 2.0) * s,
        slot.y as f32 + (m.y - 4.0) * s,
    );
    w.handle(Input::Context { x, y }, &Fixed);
    let menu = w.menu.as_ref().expect("context menu");
    let first = menu.items()[0].label.clone();
    assert_ne!(first, "");
    // The first correction replaces the word.
    w.handle(Input::Key(Key::Down, Mods::default()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    w.view.run_command(Command::SelectAll);
    let text = w.view.copy(false).map(|c| c.text).unwrap_or_default();
    assert!(text.contains(&first), "{text}");
    for _ in 0..4 {
        w.handle(Input::Tick, &Fixed);
    }
    assert_eq!(w.view.proofing_issues(), 0);
}
