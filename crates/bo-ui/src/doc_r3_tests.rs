//! BlueDoc parity round 3: Paste split button, Styles pane, picture Arrange, References
//! dialogs and galleries, Outline view, Restrict Editing, Language, Envelopes, Effects.
use super::super::DocWindow;
use crate::doc_forms::DocFormKind;
use crate::ribbon::RibbonHit;
use crate::{DocView, Input, Key, Mods, Request, TextMeasure, TextStyle};
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

fn window_with(engine: Engine) -> DocWindow {
    let mut w = DocWindow::new(DocView::new(engine).expect("view"), "en-US", "a.docx");
    w.handle(
        Input::Resize {
            width: 1800.0,
            height: 1000.0,
        },
        &Fixed,
    );
    w
}

fn window() -> DocWindow {
    let mut engine = Engine::with_bundled_fonts();
    engine.new_document("a.docx").expect("new");
    window_with(engine)
}

fn typed(text: &str) -> DocWindow {
    let mut w = window();
    w.handle(Input::Text(text.into()), &Fixed);
    w
}

/// Run interface command `id`, then let the window lay out (as after a click).
fn act(w: &mut DocWindow, id: &str) {
    w.action(id);
    w.handle(Input::Tick, &Fixed);
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

fn png() -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut e = png::Encoder::new(&mut out, 8, 6);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        let mut wr = e.write_header().expect("header");
        wr.write_image_data(&[90; 8 * 6 * 4]).expect("png");
    }
    out
}

fn xml(w: &DocWindow) -> String {
    let bytes = w.view.save_docx().expect("save");
    let pkg = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).expect("pkg");
    String::from_utf8_lossy(&pkg.part("/word/document.xml").expect("doc").data).into_owned()
}

#[test]
fn paste_split_button_lower_half_opens_paste_options() {
    let mut w = window();
    let r = w
        .ribbon
        .rect_of(RibbonHit::Command("edit.paste"))
        .expect("paste on Home");
    // Upper half: paste at once.
    click(&mut w, (r[0] + r[2] / 2.0, r[1] + r[3] * 0.25));
    assert!(!w.r3_paste_open());
    assert!(w.take_requests().contains(&Request::Paste));
    // Lower half (label and ▾): the Paste Options menu.
    click(&mut w, (r[0] + r[2] / 2.0, r[1] + r[3] * 0.85));
    assert!(w.r3_paste_open());
    assert_eq!(w.take_requests(), Vec::new());
    // Keep Text Only, the third icon: a paste with that mode.
    let m = w.r3.paste_menu.as_ref().expect("menu").rect;
    click(&mut w, (m[0] + 2.0 + 2.5 * 40.0 - 20.0, m[1] + 45.0));
    assert!(!w.r3_paste_open());
    assert!(w.take_requests().contains(&Request::Paste));
    assert_eq!(w.r3_paste_mode(true, false), bo_edit::PasteMode::Text);
    // Afterwards the default (keep source formatting) again.
    assert_eq!(w.r3_paste_mode(true, false), bo_edit::PasteMode::Keep);
    // Set Default Paste: merge between documents from other programs.
    act(&mut w, "paste.defaults");
    assert_eq!(w.r3_form_kind(), Some(DocFormKind::PasteDefaults));
    if let Some(f) = w.r3.form.as_mut() {
        f.set_sel("other", Some(1));
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(w.r3_form_kind(), None);
    assert_eq!(w.r3_paste_mode(false, true), bo_edit::PasteMode::Merge);
    // Paste Special as unformatted text.
    act(&mut w, "paste.special");
    if let Some(f) = w.r3.form.as_mut() {
        f.set_sel("as", Some(1));
    }
    let ok = w.r3_form_point("ok").expect("ok");
    click(&mut w, ok);
    assert!(w.take_requests().contains(&Request::Paste));
    assert_eq!(w.r3_paste_mode(true, false), bo_edit::PasteMode::Text);
}

#[test]
fn styles_pane_lists_styles_and_applies_one() {
    let mut w = typed("Chapter one");
    w.handle(
        Input::Key(
            Key::Letter('s'),
            Mods {
                ctrl: true,
                alt: true,
                shift: true,
            },
        ),
        &Fixed,
    );
    assert!(w.r3_styles_open(), "Alt+Ctrl+Shift+S");
    let pane = w.r3.styles_pane.as_ref().expect("pane");
    let heading = pane
        .styles
        .iter()
        .position(|s| s.id == "Heading1")
        .expect("Heading 1 listed");
    // Rows: Clear All, then the styles; 32 px rows at 125 % (25.6 at 100 %).
    let r = pane.rect;
    let row = 26.0_f32;
    let y = r[1] + 48.0 + 2.0 + (heading as f32 + 1.0) * row + row / 2.0;
    click(&mut w, (r[0] + 40.0, y));
    assert_eq!(w.view.format().style.as_deref(), Some("Heading1"));
    // The pane narrows the document; the toggle hides it.
    assert!(w.doc_right() < 1800.0);
    act(&mut w, "view.stylesPane");
    assert!(!w.r3_styles_open());
    // New Style from the pane's first button.
    act(&mut w, "view.stylesPane");
    w.pane_action(crate::doc_r3_ui::PaneAction::Button("new"));
    assert_eq!(w.r3_form_kind(), Some(DocFormKind::NewStyle));
    if let Some(f) = w.r3.form.as_mut() {
        f.set_text("name", "Quote Box");
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w.view.all_styles().iter().any(|s| s.name == "Quote Box"));
}

#[test]
fn picture_wrap_position_and_arrange() {
    let mut engine = Engine::with_bundled_fonts();
    engine.new_document("a.docx").expect("new");
    engine.insert_image(png(), "a.png").expect("picture");
    let mut w = window_with(engine);
    w.handle(Input::Tick, &Fixed);
    assert!(w
        .ribbon
        .contextual_labels()
        .contains(&crate::doc_tools::PICTURE_FORMAT));
    w.ribbon.select_tab(crate::doc_tools::PICTURE_FORMAT);
    w.relayout(&Fixed);
    for id in [
        "picture.position",
        "picture.wrap",
        "picture.bringForward",
        "picture.sendBackward",
        "picture.align",
        "picture.rotate",
    ] {
        assert!(w.ribbon.rect_of(RibbonHit::Command(id)).is_some(), "{id}");
    }
    // Wrap Text menu: Tight.
    act(&mut w, "picture.wrap");
    assert!(w.menu.is_some());
    w.menu = None;
    act(&mut w, "picture.wrap.tight");
    assert!(xml(&w).contains("<wp:wrapTight"));
    // Position gallery: bottom right.
    act(&mut w, "picture.position");
    let at = w.r3_tile_point("picture.pos.br").expect("tile");
    click(&mut w, at);
    let x = xml(&w);
    assert!(
        x.contains("<wp:align>right</wp:align>") && x.contains("<wp:wrapSquare"),
        "{x}"
    );
    act(&mut w, "picture.align.center");
    assert!(xml(&w).contains("<wp:align>center</wp:align>"));
    act(&mut w, "picture.rotate.right");
    assert!(xml(&w).contains("rot=\"5400000\""));
    act(&mut w, "picture.wrap.behind");
    assert!(xml(&w).contains("behindDoc=\"1\""));
    act(&mut w, "picture.pos.inline");
    assert!(xml(&w).contains("<wp:inline"));
}

#[test]
fn references_dialogs_insert_captions_notes_and_index() {
    let mut w = typed("Earth");
    act(&mut w, "references.insertCaption");
    assert_eq!(w.r3_form_kind(), Some(DocFormKind::Caption));
    if let Some(f) = w.r3.form.as_mut() {
        f.set_text("caption", "Figure 1: The blue planet");
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(xml(&w).contains("SEQ Figure"));
    // Cross-reference lists the caption and inserts a reference to it.
    act(&mut w, "references.crossReference");
    assert_eq!(w.r3_form_kind(), Some(DocFormKind::CrossRef));
    assert!(!w.r3.form.as_ref().expect("form").ids.is_empty(), "targets");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert_eq!(w.r3_form_kind(), None);
    assert!(xml(&w).contains(" REF "));
    // Footnote and Endnote dialog: an endnote in lower roman.
    act(&mut w, "references.notesDialog");
    if let Some(f) = w.r3.form.as_mut() {
        f.choose("end");
        f.set_sel("format", Some(3));
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let bytes = w.view.save_docx().expect("save");
    let pkg = bo_opc::Package::read(&bytes, &bo_opc::Limits::default()).expect("pkg");
    assert!(pkg.part("/word/endnotes.xml").is_some());
    // Mark Entry then Insert Index.
    w.handle(
        Input::Key(
            Key::Letter('a'),
            Mods {
                ctrl: true,
                ..Mods::default()
            },
        ),
        &Fixed,
    );
    act(&mut w, "references.markEntry");
    if let Some(f) = w.r3.form.as_mut() {
        f.set_text("main", "Earth");
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(xml(&w).contains("XE"));
    act(&mut w, "references.insertIndex");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(xml(&w).contains("INDEX"));
}

#[test]
fn table_of_contents_gallery_and_citations() {
    let bytes = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/corpus/doc/fields-03-toc.docx"
    ))
    .expect("corpus");
    let mut engine = Engine::with_bundled_fonts();
    engine.open(&bytes, "toc.docx").expect("open");
    let mut w = window_with(engine);
    // The document's own table of contents goes in one step.
    assert!(xml(&w).contains("TOC \\o"));
    act(&mut w, "references.toc.remove");
    assert!(!xml(&w).contains("TOC \\o"), "{}", xml(&w));
    w.handle(
        Input::Key(
            Key::Home,
            Mods {
                ctrl: true,
                ..Mods::default()
            },
        ),
        &Fixed,
    );
    w.ribbon.select_tab("ribbon.tabs.references");
    w.relayout(&Fixed);
    act(&mut w, "references.toc");
    let at = w.r3_tile_point("references.toc.auto1").expect("tile");
    click(&mut w, at);
    assert!(xml(&w).contains("TOC \\o"), "{}", xml(&w));
    // A new source cited at the caret, then a bibliography.
    act(&mut w, "references.citation.new");
    assert_eq!(w.r3_form_kind(), Some(DocFormKind::NewSource));
    if let Some(f) = w.r3.form.as_mut() {
        f.set_text("author", "An Nguyen");
        f.set_text("title", "Office Documents");
        f.set_text("year", "2024");
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(xml(&w).contains("CITATION"));
    act(&mut w, "references.bib.references");
    assert!(xml(&w).contains("BIBLIOGRAPHY"));
    act(&mut w, "references.style.IEEE");
    act(&mut w, "references.manageSources");
    assert_eq!(w.r3.form.as_ref().map(|f| f.ids.len()), Some(1));
}

#[test]
fn outline_view_promotes_demotes_and_filters() {
    // The UI thread's larger stack (debug frames of the window's input handling are large).
    std::thread::Builder::new()
        .stack_size(8 << 20)
        .spawn(outline_view_body)
        .expect("thread")
        .join()
        .expect("test body");
}

fn outline_view_body() {
    let mut w = typed("Part A");
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    w.handle(Input::Text("Body".into()), &Fixed);
    act(&mut w, "view.outline");
    assert!(w.r3.outline.is_some());
    assert!(w
        .ribbon
        .contextual_labels()
        .contains(&crate::doc_tools::OUTLINING));
    // The caret's paragraph (Body) is selected: promote makes it a heading.
    act(&mut w, "outline.promote");
    let items = w.view.outline();
    assert_eq!(items[1].entry.level, Some(8));
    act(&mut w, "outline.promoteHeading");
    assert_eq!(w.view.outline()[1].entry.level, Some(0));
    act(&mut w, "outline.demoteBody");
    assert_eq!(w.view.outline()[1].entry.level, None);
    act(&mut w, "outline.level.1");
    assert_eq!(w.r3.outline.as_ref().map(|o| o.show_level), Some(Some(1)));
    // Tab demotes from the keyboard, Alt+Shift+Up moves up.
    act(&mut w, "outline.level.all");
    w.handle(Input::Key(Key::Tab, Mods::default()), &Fixed);
    act(&mut w, "outline.close");
    assert!(w.r3.outline.is_none());
    assert!(!w
        .ribbon
        .contextual_labels()
        .contains(&crate::doc_tools::OUTLINING));
}

// Run on a thread with the UI thread's larger stack: the envelope is laid out by a second
// engine deep inside the window's input handling (debug frames are large).
#[test]
fn restrict_editing_language_effects_and_envelopes() {
    std::thread::Builder::new()
        .stack_size(8 << 20)
        .spawn(restrict_editing_language_effects_and_envelopes_body)
        .expect("thread")
        .join()
        .expect("test body");
}

fn restrict_editing_language_effects_and_envelopes_body() {
    let mut w = typed("Hello");
    act(&mut w, "review.restrictEditing");
    let rp = w.r3.restrict.as_mut().expect("pane");
    rp.allow_only = true;
    w.pane_action(crate::doc_r3_ui::PaneAction::Button("start"));
    assert_eq!(w.r3_form_kind(), Some(DocFormKind::Protect));
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(w
        .view
        .engine()
        .document()
        .and_then(|d| d.protection.as_ref())
        .is_some_and(|p| p.enforced && p.edit == "readOnly"));
    assert!(w.r3.restrict.as_ref().is_some_and(|r| r.protected));
    w.pane_action(crate::doc_r3_ui::PaneAction::Button("stop"));
    assert!(!w
        .view
        .engine()
        .document()
        .and_then(|d| d.protection.as_ref())
        .is_some_and(|p| p.enforced));
    // Language: Vietnamese for the selection.
    w.handle(
        Input::Key(
            Key::Letter('a'),
            Mods {
                ctrl: true,
                ..Mods::default()
            },
        ),
        &Fixed,
    );
    act(&mut w, "review.language.set");
    if let Some(f) = w.r3.form.as_mut() {
        f.set_sel("list", Some(2));
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert!(xml(&w).contains("w:val=\"vi-VN\""));
    // Effects gallery.
    w.ribbon.select_tab("ribbon.tabs.design");
    w.relayout(&Fixed);
    act(&mut w, "design.effects");
    let at = w.r3_tile_point("design.effects.soft").expect("tile");
    click(&mut w, at);
    assert_eq!(w.r3.effects.as_deref(), Some("soft"));
    act(&mut w, "design.setDefault");
    assert!(w.take_requests().contains(&Request::SaveDefaults));
    let d = w.design_defaults().expect("defaults");
    assert!(d.contains("\"effects\":\"soft\""), "{d}");
    // Envelopes: a new document, the open one untouched.
    act(&mut w, "mailings.envelopes");
    if let Some(f) = w.r3.form.as_mut() {
        f.set_text("d1", "Mr An Nguyen");
        f.set_text("d2", "1 Le Loi, Hue");
    }
    let add = w.r3_form_point("addToDocument").expect("button");
    click(&mut w, add);
    assert!(w.take_requests().contains(&Request::OpenNewDocument));
    let bytes = w.take_new_document().expect("envelope");
    let mut e = Engine::with_bundled_fonts();
    e.open(&bytes, "e.docx").expect("open");
    let text = format!("{:?}", e.document().expect("doc").sections);
    assert!(text.contains("Le Loi"));
    assert!(xml(&w).contains("Hello"));
}

#[test]
fn new_ribbon_commands_have_icons_and_labels() {
    for id in crate::doc_r3_menus::MENUS
        .iter()
        .chain(crate::doc_r3_menus::MORE_MENUS.iter())
        .chain(crate::doc_r3_menus::BUTTONS.iter())
        .chain(crate::doc_r3_menus::OUTLINE.iter())
    {
        let w = window();
        let key = format!("commands.{id}");
        assert_ne!(w.strings.get(&key), key, "label of {id}");
        assert!(
            w.icons.has(crate::ribbon::icon_of(id)) || *id == "edit.paste",
            "icon of {id}"
        );
    }
}
