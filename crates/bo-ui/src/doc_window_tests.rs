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
    // From the top (a menu opens on its current entry), then down to `entry`.
    for _ in 0..12 {
        w.handle(Input::Key(Key::Up, Mods::default()), &Fixed);
    }
    for _ in 0..entry {
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
    // Data ▾ › Repeat Header Rows.
    click(&mut w, "table.data");
    assert!(w.menu.is_some(), "Data opens its menu");
    w.menu = None;
    act(&mut w, "table.headerRow");
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
                // Check boxes and gallery tiles are drawn by the window and run with its
                // table look state.
                let drawn = crate::doc_table::labeled_w(id).is_some()
                    || crate::doc_table::large_w(id).is_some();
                if drawn {
                    if id.starts_with("table.look.") || id.starts_with("table.opt.") {
                        let mut look = crate::doc_table::TableLook::default();
                        assert!(look.command(id).is_some(), "{id} does nothing");
                    }
                    continue;
                }
                assert!(
                    drawn || w.icons.has(crate::ribbon::icon_of(id)),
                    "icon of {id}"
                );
                assert!(
                    catalog::is_menu(id)
                        || crate::doc_table::DIALOGS.contains(id)
                        || doc_tools::command(
                            id,
                            &format,
                            story,
                            0,
                            &bo_engine::markup::MarkupOptions::default()
                        )
                        .is_some()
                        || catalog::command(id, &format.section).is_some(),
                    "{id} does nothing"
                );
                for e in catalog::menu(id) {
                    assert!(
                        doc_tools::command(
                            e.id,
                            &format,
                            story,
                            0,
                            &bo_engine::markup::MarkupOptions::default()
                        )
                        .is_some()
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

#[test]
fn f7_opens_the_proofing_pane_and_walks_the_issues() {
    let mut w = window();
    w.handle(
        Input::Text("The houze is big and teh car is red".into()),
        &Fixed,
    );
    w.handle(Input::Key(Key::Home, Mods::default()), &Fixed);
    w.handle(Input::Key(Key::F(7), Mods::default()), &Fixed);
    let pane = w.spell_pane.as_ref().expect("pane");
    assert_eq!(pane.current().map(|i| i.word.as_str()), Some("houze"));
    assert!(w.view.has_selection(), "the issue is selected");
    // The document area makes room for the pane on the right.
    let right = w.doc_right();
    assert!(w
        .view
        .slots()
        .iter()
        .all(|s| (s.x + s.width) as f32 <= right));
    let first = pane.current().unwrap().suggestions[0].clone();
    w.spell_pane_action(SpellAction::Change(0));
    w.view.run_command(Command::SelectAll);
    let text = w.view.copy(false).map(|c| c.text).unwrap_or_default();
    assert!(text.contains(&first), "{text}");
    let pane = w.spell_pane.as_ref().unwrap();
    assert_eq!(pane.current().map(|i| i.word.as_str()), Some("teh"));
    w.spell_pane_action(SpellAction::IgnoreOnce);
    assert!(w.spell_pane.as_ref().unwrap().current().is_none());
    w.spell_pane_action(SpellAction::Close);
    assert!(w.spell_pane.is_none());
}

#[test]
fn review_markup_and_accept_menus() {
    use bo_engine::markup::MarkupMode;
    let mut w = window();
    show_tab(&mut w, "ribbon.tabs.review");
    click(&mut w, "review.trackChanges");
    assert!(w.view.track_changes());
    w.handle(Input::Text("tracked".into()), &Fixed);
    click(&mut w, "review.markupMode");
    // The current mode (all markup) is ticked; choose Simple.
    assert!(w.menu.as_ref().unwrap().items()[1].checked);
    choose(&mut w, 0);
    assert_eq!(w.view.markup().mode, MarkupMode::Simple);
    click(&mut w, "review.showMarkup");
    choose(&mut w, 0);
    assert!(!w.view.markup().comments);
    // Accept all: the insertion is no longer a revision, so rejecting all keeps it.
    click(&mut w, "review.acceptMenu");
    choose(&mut w, 1);
    click(&mut w, "review.rejectMenu");
    choose(&mut w, 1);
    w.view.run_command(Command::SelectAll);
    let text = w.view.copy(false).map(|c| c.text).unwrap_or_default();
    assert!(text.contains("tracked"), "{text}");
}

#[test]
fn insert_table_grid_picks_the_size_by_pointing() {
    let mut w = window();
    show_tab(&mut w, "ribbon.tabs.insert");
    click(&mut w, "insert.table");
    assert!(w.table_grid.is_some() && w.menu.is_none());
    let [ax, ay, _, ah] = w
        .ribbon
        .rect_of(RibbonHit::Command("insert.table"))
        .unwrap();
    // Third column, second row of 19 px cells under a 28 px title (scale 1).
    let (x, y) = (ax + 8.0 + 2.5 * 19.0, ay + ah + 32.0 + 1.5 * 19.0);
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
    assert!(w.table_grid.is_none());
    let t = w.view.format().table.clone().expect("table inserted");
    assert_eq!((t.cols, t.rows), (3, 2));
    assert_eq!(w.ribbon.active_label(), doc_tools::TABLE_DESIGN);
}

#[test]
fn view_tab_layouts_and_page_fit_zooms() {
    let mut w = window();
    show_tab(&mut w, "ribbon.tabs.view");
    assert!(w.on_ids().contains(&"view.printLayout"));
    click(&mut w, "view.pageWidth");
    let slot = w.view.slots()[0];
    let (_, _, view_h) = w.view.scroll_info();
    assert!(w.view.zoom() > 1.0, "page width zooms in a wide window");
    // The page spans the view but for the gaps.
    let pane = w.size.0 - w.sb_w() - w.vr_w();
    assert!(
        (pane - slot.width as f32) < 80.0,
        "{pane} vs {}",
        slot.width
    );
    click(&mut w, "view.onePage");
    let slot = w.view.slots()[0];
    assert!(slot.height as f32 <= view_h, "whole page in view");
    click(&mut w, "view.draft");
    assert_eq!(w.view_mode, "draft");
    assert!(w.on_ids().contains(&"view.draft"));
}

#[test]
fn page_setup_dialog_sets_margins_and_paper_in_centimetres() {
    let mut w = window();
    act(&mut w, "layout.pageSetup");
    let d = w.dialog.as_ref().expect("page setup dialog");
    assert_eq!(d.kind, DialogKind::PageSetup);
    // Replace the top margin (first field) by 3 cm and confirm.
    for _ in 0..8 {
        w.handle(Input::Key(Key::Backspace, Mods::default()), &Fixed);
    }
    w.handle(Input::Text("3".into()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.dialog.is_none());
    assert_eq!(w.view.format().section.margin_top, 1701);
}

#[test]
fn double_click_keeps_the_format_painter_on() {
    let mut w = window();
    w.handle(Input::Text("one two three".into()), &Fixed);
    w.view.run_command(Command::SelectAll);
    w.view.run_command(Command::FormatText {
        format: CharFormat::Bold,
    });
    let [x, y, bw, bh] = w
        .ribbon
        .rect_of(RibbonHit::Command("format.painter"))
        .unwrap();
    let (x, y) = (x + bw / 2.0, y + bh / 2.0);
    for count in [1, 2] {
        w.handle(
            Input::Press {
                x,
                y,
                count,
                shift: false,
            },
            &Fixed,
        );
        w.handle(Input::Release { x, y }, &Fixed);
    }
    assert!(w.painter && w.painter_lock);
    // Applying keeps it on; Escape turns it off.
    let slot = w.view.slots()[0];
    let (px, py) = ((slot.x + 200) as f32, (slot.y + 140) as f32);
    w.handle(
        Input::Press {
            x: px,
            y: py,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Release { x: px, y: py }, &Fixed);
    assert!(w.painter, "still on after applying");
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(!w.painter && !w.painter_lock);
}

#[test]
fn ctrl_g_goes_to_a_page() {
    let mut w = window();
    for _ in 0..3 {
        w.view.run_command(Command::InsertPageBreak);
    }
    assert_eq!(w.view.page_count(), 4);
    let ctrl = Mods {
        ctrl: true,
        ..Mods::default()
    };
    w.handle(Input::Key(Key::Letter('g'), ctrl), &Fixed);
    assert_eq!(w.find_dialog.as_ref().map(|d| d.tab), Some(2), "Go To tab");
    w.view.scroll_to(0.0);
    w.handle(Input::Text("3".into()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let (scroll, _, _) = w.view.scroll_info();
    let third = w.view.slots().iter().find(|s| s.index == 2).map(|s| s.y);
    assert!(scroll > 0.0, "scrolled to page 3");
    assert!(third.is_some(), "page 3 visible");
}

#[test]
fn word_count_lists_characters_and_paragraphs() {
    let mut w = window();
    w.handle(Input::Text("ab cd".into()), &Fixed);
    let lines = w.word_count_lines();
    assert_eq!(
        lines,
        vec![
            "Pages: 1",
            "Words: 2",
            "Characters (no spaces): 4",
            "Characters (with spaces): 5",
            "Paragraphs: 1",
        ]
    );
}

/// Table Design: a quick look and the style options; Table Layout: the nine alignment
/// buttons and the Select menu, clicked on the ribbon.
#[test]
fn table_looks_options_alignment_and_select() {
    let mut w = window();
    act(&mut w, "insert.table.3x3");
    show_tab(&mut w, doc_tools::TABLE_DESIGN);
    click(&mut w, "table.look.gridBlue");
    let shading = |w: &DocWindow| {
        w.view
            .format()
            .table
            .as_ref()
            .and_then(|t| t.shading.clone())
    };
    // The caret's cell (top left) is in the header row.
    assert_eq!(shading(&w).as_deref(), Some("B9D2EE"));
    // Without header row and first column it is the first banded row.
    click(&mut w, "table.opt.headerRow");
    click(&mut w, "table.opt.firstColumn");
    assert_eq!(shading(&w).as_deref(), Some("E8F0FA"));
    assert!(!w.table_look.option("table.opt.headerRow"));
    // The tiles and boxes are drawn over their placeholders.
    let mut rec = Rects::default();
    w.paint(&mut rec);
    assert!(rec.0 > 100);

    show_tab(&mut w, doc_tools::TABLE_LAYOUT);
    click(&mut w, "table.align.bottomRight");
    let t = w.view.format().table.clone().expect("in a table");
    assert_eq!(t.v_align, Some(bo_model::CellVAlign::Bottom));
    assert_eq!(t.cell_h_align, bo_model::Alignment::Right);
    assert!(w.on_ids().contains(&"table.align.bottomRight"));
    // Cell Size: type a row height, step the column width with its up arrow.
    click(&mut w, "table.rowHeight");
    assert!(w.cell_spin.is_some(), "the height box takes typing");
    w.handle(Input::Text("1,5".into()), &Fixed);
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.cell_spin.is_none());
    let t = w.view.format().table.clone().expect("in a table");
    assert_eq!(t.row_height, 850, "1.5 cm");
    let before = t.col_width;
    let r = w
        .ribbon
        .rect_of(RibbonHit::Command("table.colWidth"))
        .expect("width box");
    let f = crate::doc_table::spin_rect(r, w.scale);
    let (x, y) = (f[0] + f[2] - 3.0, f[1] + 3.0);
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
    let after = w.view.format().table.as_ref().map_or(0, |t| t.col_width);
    assert_eq!(after, before + 57, "one 0.1 cm step");
    // Select › Select Table: every cell.
    click(&mut w, "table.select");
    assert!(w.menu.is_some(), "Select opens its menu");
    w.menu = None;
    act(&mut w, "table.select.table");
    assert!(w.view.format().table.as_ref().is_some_and(|t| t.multi_cell));
}

/// Read Mode: no ribbon, two pages side by side fitted to the window; Esc goes back to
/// Print Layout at the zoom it had.
#[test]
fn read_mode_shows_two_pages_without_the_ribbon() {
    let mut w = window();
    let zoom = w.view.zoom();
    act(&mut w, "view.readMode");
    assert!((w.view.zoom() - 1.4).abs() < 1e-3, "reading zoom");
    assert!((w.view_top() - w.title.height()).abs() < 0.5, "no ribbon");
    assert!(w.on_ids().contains(&"view.readMode"));
    assert!(
        w.view.spell_marks().is_empty(),
        "no proofing marks while reading"
    );
    assert!(!w.view.spelling_out_of_view());
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!((w.view.zoom() - zoom).abs() < 1e-3);
    assert!(w.view_top() > w.title.height() + 10.0, "ribbon back");
    // The status bar's first view button is Read Mode, as on the reference.
    assert_eq!(VIEW_MODES[0].0, "read");
}

/// Resting on a ribbon button shows its name with the shortcut and a description.
#[test]
fn tooltips_have_a_title_shortcut_and_description() {
    let mut w = window();
    let [x, y, bw, bh] = w
        .ribbon
        .rect_of(RibbonHit::Command("format.bold"))
        .expect("bold");
    w.handle(
        Input::Move {
            x: x + bw / 2.0,
            y: y + bh / 2.0,
        },
        &Fixed,
    );
    for _ in 0..TOOLTIP_TICKS {
        w.handle(Input::Tick, &Fixed);
    }
    let tip = w.tip.clone().expect("tooltip");
    assert_eq!(tip.title, "Bold (Ctrl+B)");
    assert!(!tip.lines.is_empty(), "description under the title");
    let mut rec = Rects::default();
    w.paint(&mut rec);
    assert!(rec.0 > 0);
}

#[derive(Default)]
struct Rects(usize);
impl Painter for Rects {
    fn clear(&mut self, _: Rgba) {}
    fn fill_rect(&mut self, _: f32, _: f32, _: f32, _: f32, _: Rgba) {
        self.0 += 1;
    }
    fn page(&mut self, _: usize, _: i32, _: i32) {}
    fn text(&mut self, _: &str, _: f32, _: f32, _: TextStyle, _: Rgba) {}
    fn path(&mut self, _: &[PathSeg], _: Option<Rgba>, _: Option<(Rgba, f32)>) {}
    fn push_clip(&mut self, _: f32, _: f32, _: f32, _: f32) {}
    fn pop_clip(&mut self) {}
}

/// The navigation pane's Pages tab shows page thumbnails drawn by the host's page store.
#[test]
fn pages_tab_shows_page_thumbnails() {
    #[derive(Default)]
    struct Keys(Vec<(usize, f32)>);
    impl crate::PageStore for Keys {
        fn update(
            &mut self,
            index: usize,
            _: &bo_render::PageList,
            _: &Engine,
            scale: f32,
            _: Option<[u32; 4]>,
        ) -> Result<(), String> {
            self.0.push((index, scale));
            Ok(())
        }
        fn remove(&mut self, _: usize) {}
    }
    let mut w = window();
    act(&mut w, "view.navigationPane");
    if let Some(nav) = w.nav.as_mut() {
        nav.show(crate::nav_pane::NavTab::Pages);
    }
    w.handle(Input::Tick, &Fixed);
    let mut store = Keys::default();
    w.prepare(&mut store).unwrap();
    let thumb = store.0.iter().find(|(k, _)| *k == crate::THUMB_KEY);
    assert!(
        thumb.is_some_and(|(_, s)| *s > 0.05 && *s < 0.5),
        "{:?}",
        store.0
    );
    let rects = w.nav.as_ref().map(NavPane::thumb_rects).unwrap_or_default();
    assert_eq!(rects.len(), 1);
    let mut rec = Rects::default();
    w.paint(&mut rec);
}

/// Sizes follow the region's unit as the reference does: inches with a point in the US
/// system (0.35" for a 504-twip row), centimetres elsewhere.
#[test]
fn sizes_in_the_region_unit() {
    let us = crate::doc_table::Units {
        inches: true,
        decimal: '.',
    };
    assert_eq!(us.fmt(504), "0.35\"");
    assert_eq!(us.parse("1.63\""), Some(2347));
    assert_eq!(us.step(), 144);
    let vn = crate::doc_table::Units::default();
    assert_eq!(vn.fmt(567), "1,00 cm");
    assert_eq!(vn.parse("1,5"), Some(850));
    // An automatic row shows its laid-out height.
    let mut w = window();
    act(&mut w, "insert.table.2x2");
    w.handle(Input::Tick, &Fixed);
    for _ in 0..6 {
        w.handle(Input::Tick, &Fixed);
    }
    assert!(
        w.row_measured.is_some_and(|h| h > 200),
        "{:?}",
        w.row_measured
    );
}
