//! BlueEditor round 7: typing that makes a paragraph grow, Comment › Text Box, File page.
use crate::editor_tests::Fixed;
use crate::editor_window::EditorWindow;
use crate::{Input, Key, Mods};
use bo_pdfread::PdfDocument;

/// A PDF of `objs` (object 1 first) with a cross-reference table.
pub(crate) fn build(objs: &[String]) -> Vec<u8> {
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    out
}

/// A Letter page: a three-line paragraph, a second one close below, a rule and a paragraph
/// after a large gap.
fn paragraphs_pdf() -> Vec<u8> {
    let content = "BT /F1 12 Tf 15 TL 72 700 Td (The first paragraph has three lines of text) Tj \
                   T* (so that typing at its end makes it grow by a) Tj T* (line or two.) Tj ET \
                   BT /F1 12 Tf 15 TL 72 647 Td (The second paragraph starts close below it) Tj \
                   T* (in the same column.) Tj ET \
                   0 0 0 RG 1 w 72 600 m 400 600 l S \
                   BT /F1 12 Tf 72 540 Td (A paragraph far below, after a rule.) Tj ET";
    build(&[
        "<< /Type /Catalog /Pages 2 0 R >>".into(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".into(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R \
         /Resources << /Font << /F1 5 0 R >> >> >>"
            .into(),
        format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into(),
    ])
}

fn window_with(bytes: Vec<u8>) -> EditorWindow {
    let doc = PdfDocument::open(bytes, bo_pdfread::standard_fonts()).expect("pdf");
    let mut w = EditorWindow::new("en-US");
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1500.0,
            height: 1100.0,
        },
        &Fixed,
    );
    w.open(doc, "paras.pdf", None);
    w
}

fn top_of(w: &mut EditorWindow, start: &str) -> f32 {
    let v = w.view_mut().expect("view");
    let objs = v.doc_mut().page_objects(0).expect("objects");
    objs.iter()
        .filter(|o| o.text.starts_with(start))
        .map(|o| o.bbox[1])
        .next()
        .unwrap_or_else(|| panic!("no {start}"))
}

fn rule_top(w: &mut EditorWindow) -> f32 {
    let v = w.view_mut().expect("view");
    let objs = v.doc_mut().page_objects(0).expect("objects");
    objs.iter()
        .find(|o| o.kind == bo_pdfread::ObjectKind::Path)
        .expect("rule")
        .bbox[1]
}

/// Edit Text into the first paragraph, caret at its end, and type `s`.
fn type_at_end(w: &mut EditorWindow, s: &str) {
    if w.edit_tool() != Some(crate::EditTool::Text) {
        w.run("ed.editText", &Fixed);
    }
    let (x, y) = {
        let v = w.view_mut().expect("view");
        let objs = v.doc_mut().page_objects(0).expect("objects");
        let b = bo_pdfread::text_blocks(&objs).remove(0);
        let r = v.doc_rect_to_window(0, b.bbox).expect("on screen");
        (r[0] + 3.0, r[1] + 3.0)
    };
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
    w.handle(
        Input::Key(
            Key::End,
            Mods {
                ctrl: true,
                shift: false,
                alt: false,
            },
        ),
        &Fixed,
    );
    for c in s.chars() {
        w.handle(Input::Text(c.to_string()), &Fixed);
    }
}

/// `BO_PARAS=<file.pdf>`: type a long sentence at the end of its first paragraph and print the
/// blocks (diagnostics).
#[test]
fn debug_typing_in_file() {
    let Ok(p) = std::env::var("BO_PARAS") else {
        return;
    };
    let mut w = window_with(std::fs::read(p).expect("file"));
    let mut store = crate::editor_tests::Store::default();
    while w.prepare(&mut store) {}
    w.run("ed.editText", &Fixed);
    while w.prepare(&mut store) {}
    let (x, y) = {
        let v = w.view_mut().expect("view");
        let objs = v.doc_mut().page_objects(0).expect("objects");
        let b = bo_pdfread::text_blocks(&objs).remove(0);
        let last = b.char_boxes.last().copied().expect("chars");
        let r = v
            .doc_rect_to_window(0, [last[2], last[1], last[2], last[3]])
            .expect("on screen");
        (r[0] + 1.0, r[1] + 3.0)
    };
    press_at(&mut w, (x, y));
    for c in " 1234567890 abcdefghij klmnopqrst uvwxyzABCD EFGHIJKLMN OPQRSTUVWX YZ".chars() {
        w.handle(Input::Text(c.to_string()), &Fixed);
        w.handle(Input::Blink, &Fixed);
        w.handle(Input::Tick, &Fixed);
        let mut n = 0;
        while w.prepare(&mut store) {
            n += 1;
        }
        let shift = w
            .edit
            .typing
            .as_ref()
            .and_then(|t| t.block.as_ref())
            .map(bo_pdfread::BlockEdit::follower_shift);
        let v = w.view_mut().expect("view");
        let p = v
            .doc_mut()
            .render(0, bo_pdfread::RenderOptions::default())
            .expect("renders");
        let text = bo_pdfread::TextLayer::new(p.chars).text();
        eprintln!(
            "{c:?} prepares {n} shift {shift:?} has-second {}",
            text.contains("The second")
        );
    }
    let v = w.view_mut().expect("view");
    let objs = v.doc_mut().page_objects(0).expect("objects");
    for b in bo_pdfread::text_blocks(&objs) {
        eprintln!("{:?} {:?}", b.bbox, b.text);
    }
    eprintln!("status {:?}", w.edit.status);
}

/// A store that keeps the text of the last page drawn (whole or band).
#[derive(Default)]
struct TextStore {
    text: String,
    draws: usize,
}

impl TextStore {
    fn note(&mut self, list: &bo_render::PageList) {
        self.draws += 1;
        self.text = list
            .commands
            .iter()
            .filter_map(|c| match c {
                bo_render::Command::Glyphs { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join(" ");
    }
}

impl crate::PdfStore for TextStore {
    fn update(
        &mut self,
        _: usize,
        list: &bo_render::PageList,
        _: &bo_text::FontManager,
        _: &std::collections::HashMap<String, std::sync::Arc<bo_pdfread::Image>>,
        _: f32,
    ) -> Result<(), String> {
        self.note(list);
        Ok(())
    }

    fn update_band(
        &mut self,
        _: usize,
        list: &bo_render::PageList,
        _: &bo_text::FontManager,
        _: &std::collections::HashMap<String, std::sync::Arc<bo_pdfread::Image>>,
        _: f32,
        _: [u32; 4],
    ) -> Result<(), String> {
        self.note(list);
        Ok(())
    }

    fn remove(&mut self, _: usize) {}
}

/// Every keystroke of a long Edit Text session is drawn: the page cache used to list the
/// page once per keystroke, and after 48 the page was dropped as soon as it was rendered, so
/// the window kept showing the page as it was ~46 characters in (the text was still typed).
#[test]
fn two_hundred_typed_characters_are_all_drawn_and_saved() {
    let mut w = window_with(paragraphs_pdf());
    let mut store = TextStore::default();
    while w.prepare(&mut store) {}
    type_at_end(&mut w, "");
    while w.prepare(&mut store) {}
    let mut typed = String::new();
    for k in 0..200 {
        let c = if k % 7 == 6 {
            ' '
        } else {
            char::from(b'a' + (k % 26) as u8)
        };
        typed.push(c);
        let before = store.draws;
        w.handle(Input::Text(c.to_string()), &Fixed);
        while w.prepare(&mut store) {}
        assert!(store.draws > before, "keystroke {k} not drawn");
    }
    // The last drawn page shows the end of what was typed.
    let tail: String = typed.trim_end().chars().rev().take(5).collect::<String>();
    let tail: String = tail.chars().rev().collect();
    assert!(store.text.contains(&tail), "{tail:?} not in {:?}", store.text);
    // And the saved document has all of it.
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    let saved = w
        .view_mut()
        .expect("view")
        .doc_mut()
        .save()
        .expect("saves");
    let mut doc = PdfDocument::open(saved, bo_pdfread::standard_fonts()).expect("opens");
    let objs = doc.page_objects(0).expect("objects");
    let first = bo_pdfread::text_blocks(&objs)
        .into_iter()
        .find(|b| b.text.starts_with("The first"))
        .expect("paragraph");
    let squash = |s: &str| s.split_whitespace().collect::<String>();
    assert!(squash(&first.text).ends_with(&squash(&typed)), "{}", first.text);
}

const MORE: &str =" Now a good many more words are typed here so that it wraps twice.";

#[test]
fn growing_paragraph_pushes_its_box_and_overlaps_the_rest_with_reflow_off() {
    let mut w = window_with(paragraphs_pdf());
    let second0 = top_of(&mut w, "The second");
    let far0 = top_of(&mut w, "A paragraph far");
    let rule0 = rule_top(&mut w);
    type_at_end(&mut w, MORE);
    // While typing, the next paragraph of the box moved down by whole lines (no overlap)…
    let second = top_of(&mut w, "The second");
    let moved = second - second0;
    assert!(
        moved > 14.0 && (moved / 15.0 - (moved / 15.0).round()).abs() < 0.05,
        "{moved}"
    );
    // …the rule and the paragraph after the gap stay (the reference editor overlaps them).
    assert!((rule_top(&mut w) - rule0).abs() < 0.01);
    assert!((top_of(&mut w, "A paragraph far") - far0).abs() < 0.01);
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!((top_of(&mut w, "The second") - second).abs() < 0.01);
    // One undo step for the paragraph and what moved with it.
    w.run("edit.undo", &Fixed);
    assert!((top_of(&mut w, "The second") - second0).abs() < 0.01);
}

#[test]
fn growing_paragraph_moves_everything_below_with_reflow_on() {
    let mut w = window_with(paragraphs_pdf());
    let far0 = top_of(&mut w, "A paragraph far");
    let rule0 = rule_top(&mut w);
    w.run("ed.reflow", &Fixed);
    type_at_end(&mut w, MORE);
    let d = rule_top(&mut w) - rule0;
    assert!(d > 14.0, "{d}");
    assert!((top_of(&mut w, "A paragraph far") - far0 - d).abs() < 0.5);
    // Ending the edit does not move anything a second time.
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!((rule_top(&mut w) - rule0 - d).abs() < 0.01);
}

fn press_at(w: &mut EditorWindow, (x, y): (f32, f32)) {
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

#[test]
fn text_box_is_typed_in_place_with_the_reference_look() {
    let mut w = crate::editor_tests::window(1);
    w.run("tool.textbox", &Fixed);
    let q = {
        let r = w
            .view()
            .and_then(|v| v.doc_rect_to_window(0, [100.0, 300.0, 100.0, 300.0]))
            .expect("page");
        (r[0], r[1])
    };
    press_at(&mut w, q);
    // No question box (the reader's one opened unplaced at the window's corner): a box with
    // a caret on the page.
    assert!(!w.dialog_open());
    assert!(!w.r.modal_open());
    let d = w.tw.as_ref().expect("typing");
    assert!(d.boxed);
    assert!((d.at[0] - 97.0).abs() < 0.01, "{:?}", d.at);
    let words = "A text box with enough words to wrap onto more lines";
    for c in words.chars() {
        w.handle(Input::Text(c.to_string()), &Fixed);
    }
    let lines = w.tw.as_ref().map(|d| d.lines.clone()).unwrap_or_default();
    assert!(lines.len() >= 2, "{lines:?}");
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert!(w.tw.is_none());
    let annots = w.view().expect("view").doc().annotations();
    let a = annots.last().expect("comment");
    assert_eq!(a.kind, bo_pdfread::AnnotKind::TextBox);
    // 100 pt text area plus the 1 pt border on each side.
    assert!((a.rect[2] - a.rect[0] - 102.0).abs() < 0.05, "{:?}", a.rect);
    assert_eq!(a.contents.replace('\n', " "), words);
    // Nothing typed: no comment.
    let n = annots.len();
    press_at(&mut w, (q.0, q.1 + 200.0));
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    assert_eq!(w.view().expect("view").doc().annotations().len(), n);
}

#[test]
fn page_transitions_dialog_works_like_the_reference() {
    use crate::editor_trans7::TransHit;
    let mut w = crate::editor_tests::window(3);
    w.run("view.transitions", &Fixed);
    let paint = |w: &mut EditorWindow| {
        let mut p = crate::editor_tests::Rec::default();
        w.paint(&mut p);
        p
    };
    let p = paint(&mut w);
    for label in [
        "Page Transition",
        "Page Range",
        "Transition:",
        "All Pages",
        "No Transition",
    ] {
        assert!(p.texts.iter().any(|t| t == label), "{label}: {:?}", p.texts);
    }
    // Reference size at 125 %: 530 x 470.
    let d = w.trans.as_ref().expect("dialog");
    assert!(
        (d.frame[2] - 530.0).abs() < 1.0 && (d.frame[3] - 470.0).abs() < 1.0,
        "{:?}",
        d.frame
    );
    let click = |w: &mut EditorWindow, h: TransHit| {
        let r = w
            .trans
            .as_ref()
            .and_then(|d| d.hits.iter().find(|x| x.1 == h).map(|x| x.0))
            .unwrap_or_else(|| panic!("no {h:?}"));
        w.handle(
            Input::Press {
                x: r[0] + 2.0,
                y: r[1] + 2.0,
                count: 1,
                shift: false,
            },
            &Fixed,
        );
        paint(w);
    };
    // Direction and Speed are off while No Transition is chosen; Selected Pages too (none).
    click(&mut w, TransHit::Combo(1));
    assert_eq!(w.trans.as_ref().and_then(|d| d.list), None);
    assert!(!w
        .trans
        .as_ref()
        .is_some_and(|d| d.hits.iter().any(|h| h.1 == TransHit::Range(0))));
    // Open the Transition list and pick Wipe (the fourth style).
    click(&mut w, TransHit::Combo(0));
    click(&mut w, TransHit::Item(0, 4));
    assert_eq!(w.trans.as_ref().map(|d| d.kind), Some(4));
    // Pages From 2 To 3.
    click(&mut w, TransHit::Range(2));
    click(&mut w, TransHit::Spin(0, true));
    click(&mut w, TransHit::Spin(1, true));
    click(&mut w, TransHit::Spin(1, true));
    click(&mut w, TransHit::Ok);
    assert!(w.trans.is_none());
    let doc = w.view().expect("view").doc();
    assert_eq!(doc.page_transition(0), None);
    let t = doc.page_transition(2).expect("transition");
    assert_eq!(t.kind, bo_pdfread::TransitionKind::Wipe);
    assert!((t.seconds - 1.0).abs() < 0.01);
}

#[test]
fn file_page_has_the_reference_entries_and_batch_print() {
    let mut w = crate::editor_tests::window(1);
    w.run("file.backstage", &Fixed);
    let mut p = crate::editor_tests::Rec::default();
    w.paint(&mut p);
    for label in ["Action Wizard", "Batch Print", "Index", "Share", "Skins"] {
        assert!(p.texts.iter().any(|t| t == label), "{label}: {:?}", p.texts);
    }
    // Action Wizard and Index are grey (no hit box) with a tooltip.
    assert!(!w.bs.hits.iter().any(|h| h.1 == "bs.index"));
    assert_eq!(w.bs.tips.len(), 2);
    // Unchanged document: Save and Revert are grey.
    assert!(!w
        .bs
        .hits
        .iter()
        .any(|h| h.1 == "file.save" || h.1 == "bs.revert"));
    // Share and Skins pages.
    w.backstage_command("bs.skins");
    let mut p = crate::editor_tests::Rec::default();
    w.paint(&mut p);
    assert!(w.bs.hits.iter().any(|h| h.1 == "bs.skin.1"));
    w.backstage_command("bs.skin.1");
    assert_eq!(w.r.skin_index(), 1);
    // Batch Print: tick documents, then the Print dialog chooses the printer.
    w.backstage_command("bs.batch");
    let Some((_, crate::editor_actions::Ask2::Five(what))) = w.dialog2.take() else {
        panic!("batch print list");
    };
    let _ = w.take_requests();
    w.five_answer(what, crate::dialog::DialogButton::Ok, &["1".into()]);
    let req = w.take_requests();
    assert!(req.iter().any(|r| matches!(r, crate::ReaderRequest::Print)));
    assert_eq!(crate::PdfWindow::take_batch(&mut w), vec![0]);
}
