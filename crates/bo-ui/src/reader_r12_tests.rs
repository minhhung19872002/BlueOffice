//! Round 12: ribbon metrics, Select / Assistant menus, Comment / Share tab ends.
use super::{check_password12, hash_password12, Assist};
use crate::reader_tests::{pdf, Fixed};
use crate::reader_window::fill11::{decode_sigs, encode_sigs, SavedSig};
use crate::reader_window::{ReaderWindow, Tool};
use crate::{Input, Key, Mods};
use bo_pdfread::{PdfDocument, SignatureInk};

#[test]
fn signature_passwords_are_kept_as_salted_hashes() {
    let h = hash_password12("s3cret");
    assert!(!h.contains("s3cret"));
    assert!(check_password12(&h, "s3cret"));
    assert!(!check_password12(&h, "s3cre"));
    assert!(!check_password12("zz$00", "s3cret"));
    // A fresh salt each time.
    assert_ne!(h, hash_password12("s3cret"));
    let sig = SavedSig {
        ink: SignatureInk::Typed {
            text: "Bob".into(),
            face: 2,
        },
        initials: false,
        color: [0, 0, 255],
        keep_size: false,
        session: false,
        password: Some(h.clone()),
    };
    let text = encode_sigs(std::slice::from_ref(&sig));
    assert!(!text.contains("s3cret"));
    assert_eq!(decode_sigs(&text), vec![sig]);
    // Lines written before round 12 still read.
    let old = decode_sigs("T|1|0000ff|2|Al\n");
    assert_eq!(old.len(), 1);
    assert!(old[0].initials && old[0].password.is_none());
}

#[test]
fn a_protected_signature_asks_for_its_password() {
    let mut w = window();
    w.eleven.sigs = vec![SavedSig {
        ink: SignatureInk::Typed {
            text: "Bob".into(),
            face: 2,
        },
        initials: false,
        color: [0, 0, 0],
        keep_size: false,
        session: false,
        password: Some(hash_password12("pw")),
    }];
    assert!(w.ask_password12(0));
    assert!(w.eleven.placing.is_none());
    let none = Mods::default();
    w.handle(Input::Text("px".into()), &Fixed);
    w.handle(Input::Key(Key::Enter, none), &Fixed);
    // Wrong: still asking, the box emptied, nothing to place.
    assert!(w
        .twelve
        .pw_prompt
        .as_ref()
        .is_some_and(|p| p.2 && p.1.is_empty()));
    assert!(w.eleven.placing.is_none());
    w.handle(Input::Text("pw".into()), &Fixed);
    w.handle(Input::Key(Key::Enter, none), &Fixed);
    assert!(w.twelve.pw_prompt.is_none());
    assert_eq!(w.eleven.placing, Some(0));
}

fn window() -> ReaderWindow {
    let doc = PdfDocument::open(pdf(3), bo_pdfread::standard_fonts()).expect("pdf");
    let mut w = ReaderWindow::new("en-US");
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1518.0,
            height: 1036.0,
        },
        &Fixed,
    );
    w.open(doc, "test.pdf", None);
    w
}

fn click(w: &mut ReaderWindow, x: f32, y: f32) {
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
fn ribbon_buttons_follow_the_reference_metrics() {
    let w = window();
    // Hand keeps the reference's 52 px; the other large buttons are label + 10 px, 2 px apart.
    let hand = w.control_rect("tool.hand").expect("hand");
    assert!((hand[2] - 52.0).abs() < 0.6, "{hand:?}");
    let sel = w.control_rect("tool.select").expect("select");
    assert!((sel[0] - (hand[0] + hand[2]) - 2.0).abs() < 0.6, "{sel:?}");
    // Labels 19 px apart (two-line labels) at 125 %.
    assert!((w.label_pitch12(14.0) - 19.0).abs() < 0.01);
}

#[test]
fn select_label_opens_text_or_annotation_choice() {
    let mut w = window();
    let r = w.control_rect("tool.select").expect("select");
    // The icon part picks the Select tool at once.
    click(&mut w, r[0] + r[2] / 2.0, r[1] + 12.0);
    assert!(matches!(w.current_tool(), Tool::Select));
    assert!(!w.modal_open());
    // The label part opens the menu; Select Annotation keeps the Select tool, comments only.
    click(&mut w, r[0] + r[2] / 2.0, r[1] + r[3] - 10.0);
    assert!(w.modal_open());
    w.close_menu();
    w.run_command("select.annot", &Fixed);
    assert!(matches!(w.current_tool(), Tool::Select));
    assert!(w.twelve.select_annot);
    w.run_command("select.text", &Fixed);
    assert!(!w.twelve.select_annot);
}

#[test]
fn marquee_zooms_to_the_dragged_rectangle() {
    let mut w = window();
    w.set_ribbon_tab(2, &Fixed);
    assert!(w.control_rect("menu.assistant").is_some());
    w.run_command("menu.assistant", &Fixed);
    assert!(w.modal_open());
    w.close_menu();
    w.run_command("assist.marquee", &Fixed);
    assert_eq!(w.assist12(), Some(Assist::Marquee));
    let z0 = w.view().expect("view").zoom();
    let page = w.view().expect("view").page_rect(0).expect("page");
    // A rectangle a quarter of the page's width: zoom grows about four times (to fit the view).
    let (x0, y0) = (page[0] + page[2] * 0.25, page[1] + 100.0);
    let (x1, y1) = (x0 + page[2] / 4.0, y0 + 60.0);
    w.handle(
        Input::Press {
            x: x0,
            y: y0,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    w.handle(Input::Drag { x: x1, y: y1 }, &Fixed);
    w.handle(Input::Release { x: x1, y: y1 }, &Fixed);
    let z1 = w.view().expect("view").zoom();
    assert!(z1 > z0 * 2.5, "{z0} -> {z1}");
    // Another tool ends the marquee.
    w.run_command("tool.hand", &Fixed);
    assert_eq!(w.assist12(), None);
}

#[test]
fn share_online_commands_are_grey() {
    let mut w = window();
    w.set_ribbon_tab(5, &Fixed);
    for id in [
        "share.files",
        "share.sharedReview",
        "share.emailReview",
        "share.tracker",
    ] {
        assert!(w.control_rect(id).is_some(), "{id}");
        assert_eq!(w.twelve_enabled(id), Some(false), "{id}");
    }
    assert!(w.control_rect("share.mail").is_some());
}

#[test]
fn fdf_via_email_saves_then_starts_a_message() {
    let mut w = window();
    w.take_requests();
    w.run_command("comment.fdfMail", &Fixed);
    let reqs = w.take_requests();
    assert!(reqs
        .iter()
        .any(|r| matches!(r, crate::reader_window::ReaderRequest::SaveBytes { name, .. } if name.ends_with(".fdf"))));
    assert!(reqs.iter().any(
        |r| matches!(r, crate::reader_window::ReaderRequest::OpenUrl(u) if u.starts_with("mailto:"))
    ));
}
