use super::{popup_date, Part};
use crate::reader_tests::{pdf, Fixed, Rec};
use crate::reader_window::{ReaderRequest, ReaderWindow};
use crate::{Input, Key, Mods};
use bo_pdfread::{AnnotKind, NewAnnot, PdfDocument};

fn window() -> ReaderWindow {
    let doc = PdfDocument::open(pdf(2), bo_pdfread::standard_fonts()).expect("pdf");
    let mut w = ReaderWindow::new("en-US");
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1500.0,
            height: 1100.0,
        },
        &Fixed,
    );
    w.open(doc, "test.pdf", None);
    w
}

fn add(w: &mut ReaderWindow, kind: AnnotKind, rect: [f32; 4], text: &str) {
    let v = w.view_mut().expect("view");
    v.doc_mut()
        .add_annotation(&NewAnnot {
            kind,
            page: 0,
            rects: vec![rect],
            color: [255, 255, 0],
            author: "Alice".into(),
            contents: text.into(),
        })
        .expect("comment");
    w.handle(Input::Move { x: 1.0, y: 1.0 }, &Fixed);
}

/// Window point of a page point on page 1.
fn at(w: &ReaderWindow, x: f32, y: f32) -> (f32, f32) {
    let v = w.view().expect("view");
    let r = v.page_rect(0).expect("page");
    let s = v.px_per_pt();
    (r[0] + x * s, r[1] + y * s)
}

fn click(w: &mut ReaderWindow, (x, y): (f32, f32), count: u8) {
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

#[test]
fn dates_read_like_the_reference() {
    assert_eq!(
        popup_date("D:20261116135432+07'00'"),
        "11/16/2026 1:54:32 PM"
    );
    assert_eq!(popup_date("D:20260105090500Z"), "1/5/2026 9:05:00 AM");
    assert_eq!(popup_date(""), "");
}

#[test]
fn double_click_opens_a_note_drawn_on_the_page_typed_moved_and_closed() {
    let mut w = window();
    add(
        &mut w,
        AnnotKind::Note,
        [300.0, 150.0, 320.0, 170.0],
        "Hello",
    );
    // A double click on the note opens its pop-up beside it, the caret in it.
    let p = at(&w, 310.0, 160.0);
    click(&mut w, p, 2);
    let a = w.view().expect("view").doc().annotations().remove(0);
    let n = w
        .view()
        .expect("view")
        .doc()
        .popup_note(&a)
        .expect("pop-up");
    assert!(n.open);
    assert_eq!(n.rect, [320.0, 150.0, 500.0, 270.0]);
    // Drawn at 96 dpi whatever the zoom: 180 × 120 pt → 300 × 200 px at 125 %.
    let b = w.note_box(&n).expect("shown");
    assert_eq!((b[2], b[3]), (300.0, 200.0));
    let mut rec = Rec::default();
    w.paint(&mut rec);
    assert!(rec.texts.iter().any(|t| t == "Alice"));
    assert!(rec.texts.iter().any(|t| t == "Options"));
    assert!(rec.texts.iter().any(|t| t == "Hello"));
    // Typing goes into the note; Escape writes it.
    w.handle(Input::Text(" there".into()), &Fixed);
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    let a = w.view().expect("view").doc().annotations().remove(0);
    assert_eq!(a.contents, "Hello there");
    // The header drags the note; the page keeps its new place.
    let (hx, hy) = (b[0] + 40.0, b[1] + 10.0);
    assert_eq!(w.popup_hit(hx, hy).map(|h| h.1), Some(Part::Header));
    w.handle(
        Input::Press {
            x: hx,
            y: hy,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(
        Input::Drag {
            x: hx + 50.0,
            y: hy + 30.0,
        },
        &Fixed,
    );
    w.handle(
        Input::Release {
            x: hx + 50.0,
            y: hy + 30.0,
        },
        &Fixed,
    );
    let n2 = w
        .view()
        .expect("view")
        .doc()
        .popup_note(&a)
        .expect("pop-up");
    let b2 = w.note_box(&n2).expect("shown");
    assert_eq!((b2[0] - b[0], b2[1] - b[1]), (50.0, 30.0));
    // The bottom right grip makes it larger.
    let (gx, gy) = (b2[0] + b2[2] - 3.0, b2[1] + b2[3] - 3.0);
    assert_eq!(w.popup_hit(gx, gy).map(|h| h.1), Some(Part::Grip(false)));
    w.handle(
        Input::Press {
            x: gx,
            y: gy,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(
        Input::Drag {
            x: gx + 100.0,
            y: gy + 40.0,
        },
        &Fixed,
    );
    w.handle(
        Input::Release {
            x: gx + 100.0,
            y: gy + 40.0,
        },
        &Fixed,
    );
    let n3 = w
        .view()
        .expect("view")
        .doc()
        .popup_note(&a)
        .expect("pop-up");
    let b3 = w.note_box(&n3).expect("shown");
    assert_eq!((b3[2], b3[3]), (400.0, 240.0));
    // The close box closes it.
    let c = w.close_box(b3);
    click(&mut w, (c[0] + 4.0, c[1] + 4.0), 1);
    assert!(
        !w.view()
            .expect("view")
            .doc()
            .popup_note(&a)
            .expect("p")
            .open
    );
    assert!(
        w.note_box(&n3).is_some(),
        "box of the closed note is still computable"
    );
    assert!(w.popup_hit(c[0] + 4.0, c[1] + 4.0).is_none());
}

#[test]
fn open_and_close_all_popups_update_the_view() {
    let mut w = window();
    add(&mut w, AnnotKind::Note, [300.0, 150.0, 320.0, 170.0], "One");
    let a = w.view().expect("view").doc().annotations().remove(0);
    w.open_note10(&a, false);
    w.run_command("ctx.closePopups", &Fixed);
    w.handle(Input::Move { x: 1.0, y: 1.0 }, &Fixed);
    let mut rec = Rec::default();
    w.paint(&mut rec);
    assert!(!rec.texts.iter().any(|t| t == "One"));
    w.run_command("ctx.openPopups", &Fixed);
    w.handle(Input::Move { x: 1.0, y: 1.0 }, &Fixed);
    let mut rec = Rec::default();
    w.paint(&mut rec);
    assert!(rec.texts.iter().any(|t| t == "One"));
    // The comment menu offers Close Popup Note for an open note.
    let p = at(&w, 310.0, 160.0);
    w.handle(Input::Context { x: p.0, y: p.1 }, &Fixed);
    assert!(w.menu_labels9().contains(&"Close Popup Note".to_owned()));
    w.run_command("ctx.togglePopup", &Fixed);
    assert!(!w.note_open10(&a));
}

#[test]
fn selecting_text_shows_the_floating_toolbar_above_the_pointer() {
    let mut w = window();
    // "Page 1 hello world": 24 pt Helvetica with its baseline 92 pt from the top.
    let a = at(&w, 74.0, 84.0);
    let b = at(&w, 250.0, 84.0);
    w.handle(
        Input::Press {
            x: a.0,
            y: a.1,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Drag { x: b.0, y: b.1 }, &Fixed);
    w.handle(Input::Release { x: b.0, y: b.1 }, &Fixed);
    let r = w.bar_rect10().expect("toolbar");
    // Bottom left 2 px above the pointer at 125 %; 52 px high.
    assert_eq!(r[0], b.0.round());
    assert_eq!(r[1] + r[3], (b.1 - 2.0).round());
    assert_eq!(r[3], 52.0);
    // Its Highlight marks the selection.
    let hx = r[0] + w.bar_item_x(4) + 10.0;
    click(&mut w, (hx, r[1] + 20.0), 1);
    let all = w.view().expect("view").doc().annotations();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].kind, AnnotKind::Highlight);
    assert!(w.bar_rect10().is_none());
}

#[test]
fn paste_text_picture_and_comments() {
    let mut w = window();
    let p = at(&w, 100.0, 300.0);
    w.handle(Input::Context { x: p.0, y: p.1 }, &Fixed);
    assert_eq!(w.menu_dims9(), vec!["Paste in Place".to_owned()]);
    w.handle(Input::Key(Key::Escape, Mods::default()), &Fixed);
    w.run_command("ctx.paste", &Fixed);
    let req = w.take_requests();
    assert!(matches!(&req[..], [ReaderRequest::Command(c)] if c == "host.paste10"));
    assert!(w.file_picked10("paste10txt", b"Pasted line"));
    let all = w.view().expect("view").doc().annotations();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].kind, AnnotKind::TextBox);
    assert_eq!(all[0].contents, "Pasted line");
    assert!((all[0].rect[0] - 100.0).abs() < 0.5 && (all[0].rect[1] - 300.0).abs() < 0.5);
    // A picture becomes a stamp at 96 dpi: a 2 × 1 DIB.
    let mut dib = vec![0_u8; 40];
    dib[0] = 40;
    dib[4] = 2;
    dib[8] = 1;
    dib[12] = 1;
    dib[14] = 32;
    dib.extend_from_slice(&[0, 0, 255, 255, 255, 0, 0, 255]);
    w.run_command("ctx.paste", &Fixed);
    w.take_requests();
    assert!(w.file_picked10("paste10img", &dib));
    let all = w.view().expect("view").doc().annotations();
    assert_eq!(all.len(), 2);
    assert_eq!(all[1].kind, AnnotKind::Stamp);
    // Copy a comment, Paste in Place puts a copy over it.
    w.extra_select(all[0].clone());
    w.handle(
        Input::Key(
            Key::Letter('c'),
            Mods {
                ctrl: true,
                ..Mods::default()
            },
        ),
        &Fixed,
    );
    let req = w.take_requests();
    assert!(
        matches!(&req[..], [ReaderRequest::Copy(t)] if t == "Pasted line"),
        "{req:?}"
    );
    w.run_command("ctx.pasteInPlace", &Fixed);
    let all = w.view().expect("view").doc().annotations();
    assert_eq!(all.len(), 3);
    assert_eq!(all[2].rect, all[0].rect);
}
