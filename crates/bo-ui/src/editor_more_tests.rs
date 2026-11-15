//! BlueEditor round 4: Organize row buttons and page commands, Convert exports.
use crate::editor_tests::{key, pdf, texts, window, Fixed};
use crate::editor_window::TAB_ORGANIZE;
use crate::reader_window::ReaderRequest;
use crate::{Input, Key, Mods};

fn type_in(w: &mut crate::EditorWindow, fields: &[&str]) {
    // Replace each field's text and move on with Tab.
    for (k, f) in fields.iter().enumerate() {
        if k > 0 {
            w.handle(Input::Key(Key::Tab, Mods::default()), &Fixed);
        }
        let ctrl = Mods {
            ctrl: true,
            ..Mods::default()
        };
        w.handle(Input::Key(Key::Letter('a'), ctrl), &Fixed);
        for _ in 0..12 {
            w.handle(Input::Key(Key::Backspace, Mods::default()), &Fixed);
        }
        w.handle(Input::Text((*f).into()), &Fixed);
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
}

#[test]
fn organize_rows_stack_three_to_a_column() {
    let mut w = window(3);
    w.show_tab(TAB_ORGANIZE, &Fixed);
    let r = |id: &str| w.r.control_rect(id).unwrap_or_else(|| panic!("{id}"));
    let (mv, dup, rep, split) = (
        r("org.move"),
        r("org.duplicate"),
        r("org.replace"),
        r("org.split"),
    );
    assert_eq!(mv[0], dup[0]);
    assert_eq!(dup[0], rep[0]);
    assert!(mv[1] < dup[1] && dup[1] < rep[1]);
    // One width per column; the next column starts after it.
    assert_eq!(mv[2], rep[2]);
    assert!((split[0] - (mv[0] + mv[2])).abs() < 0.5);
    assert_eq!(split[1], mv[1]);
    // The rotate button comes after the second column and a separator.
    assert!(r("org.rotateMenu")[0] > split[0] + split[2]);
}

#[test]
fn duplicate_swap_move_and_page_numbers() {
    let mut w = window(3);
    w.show_tab(TAB_ORGANIZE, &Fixed);
    key(&mut w, Key::Home, false, false);
    w.run("org.duplicate", &Fixed);
    assert_eq!(texts(&mut w), ["Page 1", "Page 1", "Page 2", "Page 3"]);
    assert_eq!(w.selected_pages(), [1]);
    key(&mut w, Key::Letter('z'), true, false);
    assert_eq!(texts(&mut w).len(), 3);
    // Swap pages 1 and 3.
    w.run("org.swap", &Fixed);
    type_in(&mut w, &["1", "3"]);
    assert_eq!(texts(&mut w), ["Page 3", "Page 2", "Page 1"]);
    // Move page 1 before page 3 (it lands second).
    w.run("org.move", &Fixed);
    type_in(&mut w, &["1", "3"]);
    assert_eq!(texts(&mut w), ["Page 2", "Page 3", "Page 1"]);
    // Pages 1-2 numbered i, ii; page 3 keeps its number.
    w.run("org.pageNumbers", &Fixed);
    type_in(&mut w, &["1", "2", "i", "", "1"]);
    let labels: Vec<String> = (0..3)
        .map(|i| {
            w.view()
                .and_then(|v| v.doc().page_info(i))
                .map(|p| p.label.clone())
                .unwrap_or_default()
        })
        .collect();
    assert_eq!(labels, ["i", "ii", "3"]);
}

#[test]
fn replace_interleave_and_resize() {
    let mut w = window(2);
    w.show_tab(TAB_ORGANIZE, &Fixed);
    key(&mut w, Key::End, false, false);
    w.run("org.replace", &Fixed);
    let req = w.take_requests();
    assert!(
        matches!(&req[..], [ReaderRequest::PickFile { purpose, .. }] if purpose == "replacePages")
    );
    w.file_picked("replacePages", std::path::Path::new("x.pdf"), &pdf(3));
    // The selected last page is replaced by the first page of the file.
    assert_eq!(texts(&mut w), ["Page 1", "Page 1"]);
    key(&mut w, Key::Letter('z'), true, false);
    w.run("org.interleave", &Fixed);
    w.take_requests();
    w.file_picked("interleave", std::path::Path::new("x.pdf"), &pdf(3));
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(texts(&mut w).len(), 5);
    // Resize every page to A5 with the content scaled.
    w.run("org.resize", &Fixed);
    type_in(&mut w, &["148", "210"]);
    let size = w.view().map(|v| v.page_size(0)).expect("size");
    assert!(
        (size.0 - 419.5).abs() < 1.0 && (size.1 - 595.3).abs() < 1.0,
        "{size:?}"
    );
}

#[test]
fn convert_exports_and_new_documents() {
    let mut w = window(1);
    w.run("conv.exportImages", &Fixed);
    assert!(matches!(w.take_requests()[..], []));
    w.run("conv.html", &Fixed);
    let req = w.take_requests();
    let Some(ReaderRequest::SaveBytes { name, bytes }) = req.first() else {
        panic!("{req:?}");
    };
    assert_eq!(name, "document.html");
    assert!(String::from_utf8_lossy(bytes).contains("Page 1"));
    w.run("conv.template.Letter", &Fixed);
    let req = w.take_requests();
    assert!(matches!(&req[..], [ReaderRequest::OpenDocument { .. }]));
    // From Clipboard: the host hands over the bitmap; an empty clipboard says so.
    w.run("conv.clipboard", &Fixed);
    let req = w.take_requests();
    assert!(
        matches!(&req[..], [ReaderRequest::Command(c)] if c == "host.clipboardImage"),
        "{req:?}"
    );
    w.file_picked("clipboardImage", std::path::Path::new("c.bmp"), &[]);
    assert!(matches!(w.take_requests()[..], []));
    // A 2 × 1 bottom-up 32-bit DIB.
    let mut dib = vec![0_u8; 40];
    dib[0] = 40;
    dib[4] = 2;
    dib[8] = 1;
    dib[12] = 1;
    dib[14] = 32;
    dib.extend_from_slice(&[0, 0, 255, 255, 255, 0, 0, 255]);
    w.file_picked("clipboardImage", std::path::Path::new("c.bmp"), &dib);
    let req = w.take_requests();
    assert!(
        matches!(&req[..], [ReaderRequest::OpenDocument { .. }]),
        "{req:?} {}",
        w.r.status_text()
    );
}

#[test]
fn grey_commands_do_nothing() {
    let mut w = window(1);
    for c in ["conv.web", "conv.ocr", "org.rearrange"] {
        w.run(c, &Fixed);
    }
    assert!(matches!(w.take_requests()[..], []));
}

/// Click Edit Text into the first paragraph of page 0, at its first character.
fn start_typing(w: &mut crate::EditorWindow) {
    if w.edit_tool() != Some(crate::EditTool::Text) {
        w.run("ed.editText", &Fixed);
    }
    let v = w.view_mut().expect("view");
    let objs = v.doc_mut().page_objects(0).expect("objects");
    let b = bo_pdfread::text_blocks(&objs).remove(0);
    let r = v.doc_rect_to_window(0, b.bbox).expect("on screen");
    let (x, y) = (r[0] + 3.0, r[1] + r[3] / 2.0);
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

fn typed(w: &mut crate::EditorWindow, s: &str) {
    for c in s.chars() {
        w.handle(Input::Text(c.to_string()), &Fixed);
    }
}

#[test]
fn typing_words_selection_clipboard_and_session_undo() {
    let mut w = window(1);
    start_typing(&mut w);
    key(&mut w, Key::End, false, false);
    typed(&mut w, " one two");
    // Ctrl+Backspace deletes the word before the caret; Ctrl+Left jumps a word.
    key(&mut w, Key::Backspace, true, false);
    key(&mut w, Key::Left, true, false);
    key(&mut w, Key::Left, true, true);
    // Ctrl+C copies the selected word, Ctrl+X cuts it.
    key(&mut w, Key::Letter('c'), true, false);
    let req = w.take_requests();
    assert!(
        matches!(&req[..], [ReaderRequest::Copy(s)] if s == "1 "),
        "{req:?}"
    );
    key(&mut w, Key::Letter('x'), true, false);
    w.take_requests();
    // Paste comes from the host as plain text.
    key(&mut w, Key::End, false, false);
    key(&mut w, Key::Letter('v'), true, false);
    let req = w.take_requests();
    assert!(matches!(&req[..], [ReaderRequest::Command(c)] if c == "host.pasteText"));
    w.file_picked("pasteText", std::path::Path::new("c.txt"), b"X\tY");
    key(&mut w, Key::Escape, false, false);
    assert_eq!(texts(&mut w), ["Page one X Y"]);
    // Inside a session Ctrl+Z steps back a word at a time, Ctrl+Y forward.
    start_typing(&mut w);
    key(&mut w, Key::End, false, false);
    typed(&mut w, " ab cd");
    key(&mut w, Key::Letter('z'), true, false);
    key(&mut w, Key::Escape, false, false);
    assert_eq!(texts(&mut w), ["Page one X Y ab"]);
    start_typing(&mut w);
    key(&mut w, Key::End, false, false);
    typed(&mut w, " ef");
    key(&mut w, Key::Letter('z'), true, false);
    key(&mut w, Key::Letter('y'), true, false);
    key(&mut w, Key::Escape, false, false);
    assert_eq!(texts(&mut w), ["Page one X Y ab ef"]);
}

#[test]
#[ignore = "layout dump for measuring"]
fn dump_home_layout() {
    let w = window(1);
    for id in [
        "tool.hand",
        "tool.select",
        "home.snapshot",
        "ed.clipboard",
        "ed.bookmark",
        "sep1",
        "menu.zoom",
        "menu.fit",
        "view.reflow",
        "menu.rotate",
        "sep2",
        "ed.editText",
        "ed.editObject",
        "sep3",
    ] {
        eprintln!("{id}: {:?}", w.r.control_rect(id));
    }
}
