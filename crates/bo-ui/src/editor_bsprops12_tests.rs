//! BlueEditor round 12: File › Properties pages (Description, Initial View, Security, Fonts,
//! Advanced) edit the document.
use crate::editor_dlg8::FormWhat;
use crate::editor_form8::Val;
use crate::editor_tests::{window, Fixed, Rec};
use crate::editor_window::EditorWindow;
use crate::{Input, Key, Mods};

fn props(pages: usize) -> EditorWindow {
    let mut w = window(pages);
    w.run("file.backstage", &Fixed);
    w.backstage_command("bs.properties");
    w
}

fn texts(w: &mut EditorWindow) -> Vec<String> {
    let mut rec = Rec::default();
    w.paint(&mut rec);
    rec.texts
}

fn click_cmd(w: &mut EditorWindow, cmd: &str) {
    let all: Vec<String> = w.bs.hits.iter().map(|h| h.1.clone()).collect();
    let r =
        w.bs.hits
            .iter()
            .find(|h| h.1 == cmd)
            .map_or_else(|| panic!("{cmd}: {all:?}"), |h| h.0);
    let (x, y) = (r[0] + r[2] / 2.0, r[1] + r[3] / 2.0);
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
fn description_page_edits_the_title() {
    let mut w = props(3);
    let t = texts(&mut w);
    for l in [
        "Description",
        "Initial View",
        "Security",
        "Fonts",
        "Advanced",
        "General",
        "PDF Version",
        "add a title",
        "Related Dates",
        "Show More Information",
    ] {
        assert!(t.iter().any(|x| x == l), "{l}: {t:?}");
    }
    click_cmd(&mut w, "bsp12.info.Title");
    {
        let f = &mut w.form8.last_mut().expect("dialog").0;
        f.set("value", Val::Text("Quarterly report".into()));
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(
        w.view().expect("view").doc().doc_info().title,
        "Quarterly report"
    );
    assert!(texts(&mut w).iter().any(|x| x == "Quarterly report"));
    // Custom property.
    texts(&mut w);
    click_cmd(&mut w, "bsp12.custom");
    {
        let f = &mut w.form8.last_mut().expect("dialog").0;
        f.set("name", Val::Text("Project".into()));
        f.set("value", Val::Text("Blue".into()));
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    assert_eq!(
        w.view().expect("view").doc().custom_info(),
        [("Project".to_owned(), "Blue".to_owned())]
    );
}

#[test]
fn initial_view_is_written_into_the_document() {
    let mut w = props(3);
    texts(&mut w);
    click_cmd(&mut w, "bsp12.sub1");
    let t = texts(&mut w);
    for l in [
        "Layout and Destination",
        "Page Only",
        "Window Options",
        "Hide menu bar",
    ] {
        assert!(t.iter().any(|x| x == l), "{l}: {t:?}");
    }
    // Navigation › Bookmarks Panel and Page from the drop-down list.
    click_cmd(&mut w, "bsp12.combo.iv.nav");
    assert!(texts(&mut w).iter().any(|x| x == "Layers Panel and Page"));
    click_cmd(&mut w, "bsp12.pick.iv.nav.1");
    assert!(w.bsp12.open.is_none());
    click_cmd(&mut w, "bsp12.check.iv.center");
    texts(&mut w);
    click_cmd(&mut w, "bsp12.openPage");
    {
        let f = &mut w.form8.last_mut().expect("dialog").0;
        f.set("value", Val::Text("2".into()));
    }
    w.handle(Input::Key(Key::Enter, Mods::default()), &Fixed);
    let v = w.view().expect("view").doc().initial_view();
    assert_eq!((v.navigation, v.page, v.center_window), (1, 1, true));
    assert!(w.view().expect("view").doc().modified());
}

#[test]
fn security_fonts_and_advanced_pages() {
    let mut w = props(2);
    texts(&mut w);
    click_cmd(&mut w, "bsp12.sub2");
    let t = texts(&mut w);
    for l in [
        "No Protection",
        "Document Restrictions Summary",
        "Printing",
        "Allowed",
    ] {
        assert!(t.iter().any(|x| x == l), "{l}: {t:?}");
    }
    click_cmd(&mut w, "bsp12.sub3");
    texts(&mut w);
    click_cmd(&mut w, "bsp12.sub4");
    let t = texts(&mut w);
    for l in [
        "Print Dialog Presets",
        "Simplex",
        "Reading Options",
        "Left Edge",
    ] {
        assert!(t.iter().any(|x| x == l), "{l}: {t:?}");
    }
    click_cmd(&mut w, "bsp12.combo.adv.duplex");
    texts(&mut w);
    click_cmd(&mut w, "bsp12.pick.adv.duplex.2");
    texts(&mut w);
    click_cmd(&mut w, "bsp12.check.adv.tray");
    let p = w.view().expect("view").doc().print_presets();
    assert_eq!((p.duplex, p.tray_by_size), (2, true));
    assert!(matches!(
        w.form8.last(),
        None | Some((_, FormWhat::Delete9))
    ));
}
