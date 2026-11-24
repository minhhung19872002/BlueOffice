//! Round 25 tests: keyboard parity.
use super::super::r23::inv_tests::{window, Ctx};
use crate::ribbon::Item;
use std::fmt::Write as _;

/// `BO_DUMP25=<file>`: every ribbon tab's commands with their labels (tab, group, id, label,
/// kind) — the input of the key tip table.
#[test]
#[ignore = "writes the ribbon command list (BO_DUMP25=<file>)"]
fn dump_ribbon() {
    let Ok(path) = std::env::var("BO_DUMP25") else {
        return;
    };
    let w = window(Ctx::Text);
    let mut tabs = crate::doc_ribbon21::main_tabs();
    tabs.extend(crate::doc_ribbon21::tool_tabs());
    let mut out = String::new();
    for tab in &tabs {
        let tl = w.strings.get(tab.label).to_owned();
        for g in &tab.groups {
            let gl = w.strings.get(g.label).to_owned();
            for it in g.large.iter().chain(g.rows.iter().flatten()) {
                let (id, kind) = match it {
                    Item::Button(id) => (*id, "button"),
                    Item::Large(id) => (*id, "large"),
                    Item::Labeled(id) => (*id, "labeled"),
                    Item::FontBox => ("fontBox", "box"),
                    Item::SizeBox => ("sizeBox", "box"),
                    Item::Gallery => ("gallery", "gallery"),
                };
                let k = format!("commands.{id}");
                let l = w.strings.get(&k).to_owned();
                let _ = writeln!(out, "{tl}\t{}\t{gl}\t{id}\t{l}\t{kind}", tab.label);
            }
        }
    }
    std::fs::write(path, out).expect("write");
}

use super::super::r23::inv_tests::{effect, painted, RICH};
use crate::{Input, Key, Mods, TextMeasure, TextStyle};

struct Fixed;
impl TextMeasure for Fixed {
    fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
        (
            text.chars().count() as f32 * style.size * 0.55,
            style.size * 1.33,
        )
    }
}

/// The documented Windows shortcuts of the reference word processor (its public "Keyboard
/// shortcuts" page) with the sample they run in: `T` text (the rich sample: notes, a list, a
/// link, a field, an equation, a comment, a tracked change, a chart), `B` a table cell, `O`
/// outline view, `S` a shape selected.
pub(crate) const SHORTCUTS: &[(&str, char, &str)] = &[
    // Frequently used.
    ("Ctrl+O", 'T', "open"),
    ("Ctrl+N", 'T', "new document"),
    ("Ctrl+S", 'T', "save"),
    ("Ctrl+W", 'T', "close"),
    ("Ctrl+X", 'T', "cut"),
    ("Ctrl+C", 'T', "copy"),
    ("Ctrl+V", 'T', "paste"),
    ("Ctrl+A", 'T', "select all"),
    ("Ctrl+B", 'T', "bold"),
    ("Ctrl+I", 'T', "italic"),
    ("Ctrl+U", 'T', "underline"),
    ("Ctrl+[", 'T', "font size -1"),
    ("Ctrl+]", 'T', "font size +1"),
    ("Ctrl+E", 'T', "center"),
    ("Ctrl+L", 'T', "align left"),
    ("Ctrl+R", 'T', "align right"),
    ("Ctrl+J", 'T', "justify"),
    ("Ctrl+Z", 'T', "undo"),
    ("Ctrl+Y", 'T', "redo / repeat"),
    ("Ctrl+Alt+S", 'T', "split window"),
    ("Alt+Shift+C", 'T', "remove split / close pane"),
    ("Ctrl+P", 'T', "print (backstage)"),
    ("Ctrl+K", 'T', "hyperlink"),
    // Ribbon.
    ("F10", 'T', "key tips"),
    ("Ctrl+F1", 'T', "collapse ribbon"),
    ("Shift+F10", 'T', "context menu"),
    ("Alt+Q", 'T', "search box"),
    // Navigation.
    ("Ctrl+Left", 'T', "word left"),
    ("Ctrl+Right", 'T', "word right"),
    ("Ctrl+Up", 'T', "paragraph up"),
    ("Ctrl+Down", 'T', "paragraph down"),
    ("Ctrl+Home", 'T', "document start"),
    ("Ctrl+End", 'T', "document end"),
    ("Ctrl+Alt+PageUp", 'T', "top of screen"),
    ("Ctrl+Alt+PageDown", 'T', "bottom of screen"),
    ("Ctrl+PageUp", 'T', "previous page / browse object"),
    ("Ctrl+PageDown", 'T', "next page / browse object"),
    ("Ctrl+F", 'T', "navigation pane"),
    ("Ctrl+G", 'T', "go to"),
    ("F5", 'T', "go to"),
    ("Shift+F5", 'T', "previous edit"),
    ("Ctrl+Alt+Z", 'T', "go back"),
    // Selection.
    ("Shift+Right", 'T', "extend char"),
    ("Shift+Down", 'T', "extend line"),
    ("Ctrl+Shift+Right", 'T', "extend word"),
    ("Ctrl+Shift+Down", 'T', "extend paragraph"),
    ("Shift+End", 'T', "extend to line end"),
    ("Ctrl+Shift+End", 'T', "extend to document end"),
    ("F8", 'T', "extend mode"),
    ("Shift+F8", 'T', "shrink selection"),
    ("Ctrl+Shift+F8", 'T', "column selection"),
    // Editing.
    ("Ctrl+Backspace", 'T', "delete word left"),
    ("Ctrl+Delete", 'T', "delete word right"),
    ("F2", 'T', "move text"),
    ("Shift+F2", 'T', "copy text"),
    ("Ctrl+F3", 'T', "cut to spike"),
    ("Ctrl+Shift+F3", 'T', "insert spike"),
    ("Ctrl+Shift+C", 'T', "copy formatting"),
    ("Ctrl+Shift+V", 'T', "paste formatting"),
    ("Ctrl+Alt+V", 'T', "paste special"),
    ("Alt+F3", 'T', "create building block"),
    ("F3", 'T', "insert AutoText"),
    ("Ctrl+H", 'T', "replace"),
    ("Ctrl+M", 'T', "indent"),
    ("Ctrl+Shift+M", 'T', "outdent"),
    ("Ctrl+T", 'T', "hanging indent"),
    ("Ctrl+Shift+T", 'T', "reduce hanging indent"),
    ("Ctrl+Q", 'T', "reset paragraph"),
    ("Ctrl+1", 'T', "single spacing"),
    ("Ctrl+2", 'T', "double spacing"),
    ("Ctrl+5", 'T', "1.5 spacing"),
    ("Ctrl+0", 'T', "space before 12 pt"),
    ("Ctrl+Alt+K", 'T', "AutoFormat"),
    ("Ctrl+Shift+N", 'T', "Normal style"),
    ("Ctrl+Alt+1", 'T', "Heading 1"),
    ("Ctrl+Alt+2", 'T', "Heading 2"),
    ("Ctrl+Alt+3", 'T', "Heading 3"),
    ("Ctrl+Shift+S", 'T', "Apply Styles"),
    ("Ctrl+Alt+Shift+S", 'T', "Styles pane"),
    ("Ctrl+Shift+L", 'T', "List Bullet"),
    // Character formatting.
    ("Ctrl+D", 'T', "Font dialog"),
    ("Ctrl+Shift+F", 'T', "Font dialog / font box"),
    ("Ctrl+Shift+P", 'T', "font size box"),
    ("Ctrl+Shift+.", 'T', "grow font"),
    ("Ctrl+Shift+,", 'T', "shrink font"),
    ("Shift+F3", 'T', "change case"),
    ("Ctrl+Shift+A", 'T', "all caps"),
    ("Ctrl+Shift+H", 'T', "hidden"),
    ("Ctrl+Shift+W", 'T', "underline words"),
    ("Ctrl+Shift+D", 'T', "double underline"),
    ("Ctrl+Shift+K", 'T', "small caps"),
    ("Ctrl+=", 'T', "subscript"),
    ("Ctrl+Shift+=", 'T', "superscript"),
    ("Ctrl+Space", 'T', "clear character formatting"),
    ("Ctrl+Shift+Q", 'T', "Symbol font"),
    ("Shift+F1", 'T', "Reveal Formatting"),
    ("Ctrl+Shift+8", 'T', "show formatting marks"),
    // Special characters.
    ("Shift+Enter", 'T', "line break"),
    ("Ctrl+Enter", 'T', "page break"),
    ("Ctrl+Shift+Enter", 'T', "column break"),
    ("Ctrl+Shift+Space", 'T', "nonbreaking space"),
    ("Ctrl+-", 'T', "optional hyphen"),
    ("Ctrl+Shift+-", 'T', "nonbreaking hyphen"),
    ("Ctrl+Alt+-", 'T', "em dash"),
    ("Ctrl+Alt+C", 'T', "copyright"),
    ("Ctrl+Alt+R", 'T', "registered"),
    ("Ctrl+Alt+T", 'T', "trademark"),
    ("Ctrl+Alt+.", 'T', "ellipsis"),
    ("Alt+X", 'T', "Unicode toggle"),
    // Tables.
    ("Alt+Home", 'B', "first cell in row"),
    ("Alt+End", 'B', "last cell in row"),
    ("Alt+PageUp", 'B', "first cell in column"),
    ("Alt+PageDown", 'B', "last cell in column"),
    ("Alt+Shift+Up", 'B', "move row up"),
    ("Alt+Shift+Down", 'B', "move row down"),
    ("Ctrl+Tab", 'B', "tab in cell"),
    ("Tab", 'B', "next cell"),
    ("Shift+Tab", 'B', "previous cell"),
    // Review, references, mail merge, fields.
    ("Ctrl+Alt+M", 'T', "new comment"),
    ("Ctrl+Shift+E", 'T', "track changes"),
    ("Alt+Shift+O", 'T', "mark TOC entry"),
    ("Alt+Shift+I", 'T', "mark citation"),
    ("Alt+Shift+X", 'T', "mark index entry"),
    ("Ctrl+Alt+F", 'T', "footnote"),
    ("Ctrl+Alt+D", 'T', "endnote"),
    ("Alt+Shift+.", 'T', "next footnote"),
    ("Alt+Shift+,", 'T', "previous footnote"),
    ("Alt+Shift+K", 'T', "preview merge"),
    ("Alt+Shift+N", 'T', "merge"),
    ("Alt+Shift+M", 'T', "print merged"),
    ("Alt+Shift+E", 'T', "edit data source"),
    ("Alt+Shift+F", 'T', "merge field"),
    ("Alt+Shift+D", 'T', "DATE field"),
    ("Alt+Shift+P", 'T', "PAGE field"),
    ("Alt+Shift+T", 'T', "TIME field"),
    ("Ctrl+Alt+L", 'T', "LISTNUM field"),
    ("Ctrl+F9", 'T', "empty field"),
    ("F9", 'T', "update field"),
    ("Shift+F9", 'T', "toggle field code"),
    ("Alt+F9", 'T', "toggle all field codes"),
    ("Ctrl+Shift+F9", 'T', "unlink field"),
    ("F11", 'T', "next field"),
    ("Shift+F11", 'T', "previous field"),
    ("Ctrl+F11", 'T', "lock field"),
    ("Ctrl+Shift+F11", 'T', "unlock field"),
    // Views and outline.
    ("Ctrl+Alt+P", 'T', "Print Layout"),
    ("Ctrl+Alt+O", 'T', "Outline"),
    ("Ctrl+Alt+N", 'T', "Draft"),
    ("Ctrl+Alt+I", 'T', "print preview"),
    ("Ctrl+F2", 'T', "print preview"),
    ("Alt+Shift+Left", 'O', "promote"),
    ("Alt+Shift+Right", 'O', "demote"),
    ("Alt+Shift+A", 'O', "expand / collapse all"),
    ("Alt+Shift+L", 'O', "first line only"),
    ("Alt+Shift+1", 'O', "show Heading 1"),
    ("Alt+Shift+=", 'O', "expand"),
    ("Alt+Shift+-", 'O', "collapse"),
    // Function keys.
    ("F1", 'T', "help"),
    ("F4", 'T', "repeat"),
    ("Ctrl+F4", 'T', "close document"),
    ("Ctrl+Shift+F5", 'T', "bookmark"),
    ("F6", 'T', "next pane"),
    ("Shift+F6", 'T', "previous pane"),
    ("Ctrl+F6", 'T', "next window"),
    ("Ctrl+Shift+F6", 'T', "previous window"),
    ("F7", 'T', "spelling"),
    ("Shift+F7", 'T', "thesaurus"),
    ("Alt+F7", 'T', "next misspelling"),
    ("Alt+Shift+F7", 'T', "translate"),
    ("Ctrl+F10", 'T', "maximize"),
    ("Alt+F10", 'T', "selection pane"),
    ("F12", 'T', "Save As"),
    ("Shift+F12", 'T', "save"),
    ("Ctrl+F12", 'T', "open"),
    ("Ctrl+Shift+F12", 'T', "print"),
    ("Alt+Ctrl+F2", 'T', "open"),
    ("Alt+F8", 'T', "macros"),
    ("Alt+F11", 'T', "VBA editor"),
];

/// `spec` ("Ctrl+Shift+F3") as the window gets it from the host (`win32::key`).
pub(crate) fn parse(spec: &str) -> (Key, Mods) {
    let mut m = Mods::default();
    let mut key = None;
    let parts: Vec<&str> = if spec.ends_with("++") || spec == "+" {
        vec![]
    } else {
        spec.split('+').collect()
    };
    for p in parts {
        match p {
            "Ctrl" => m.ctrl = true,
            "Shift" => m.shift = true,
            "Alt" => m.alt = true,
            "" => key = Some(Key::Letter('-')),
            k => key = Some(named(k)),
        }
    }
    // "Ctrl+=" / "Alt+Shift+=": the host reports the plus key as '+'.
    (key.expect("key"), m)
}

fn named(k: &str) -> Key {
    match k {
        "Left" => Key::Left,
        "Right" => Key::Right,
        "Up" => Key::Up,
        "Down" => Key::Down,
        "Home" => Key::Home,
        "End" => Key::End,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,
        "Backspace" => Key::Backspace,
        "Delete" => Key::Delete,
        "Enter" => Key::Enter,
        "Tab" => Key::Tab,
        "Esc" => Key::Escape,
        "Space" => Key::Letter(' '),
        "=" => Key::Letter('+'),
        f if f.starts_with('F') && f.len() > 1 => Key::F(f[1..].parse().expect("F-key")),
        c => Key::Letter(c.chars().next().expect("char").to_ascii_lowercase()),
    }
}

/// A fresh window for a shortcut's sample.
pub(crate) fn sample(ctx: char) -> super::super::DocWindow {
    RICH.with(|r| r.set(ctx == 'T'));
    let mut w = match ctx {
        'B' => window(Ctx::Table),
        'S' => window(Ctx::Shape),
        _ => window(Ctx::Text),
    };
    RICH.with(|r| r.set(false));
    if ctx == 'O' {
        let _ = w.action("view.outline");
        w.handle(Input::Tick, &Fixed);
        let _ = w.take_requests();
    }
    w
}

/// Every documented shortcut with what it did in a fresh window.
pub(crate) fn shortcut_table() -> String {
    let mut out = String::from("keys\tsample\twhat\teffect\n");
    for (spec, ctx, what) in SHORTCUTS {
        let mut w = sample(*ctx);
        let before = painted(&mut w);
        let (k, m) = parse(spec);
        let ret = w.handle(Input::Key(k, m), &Fixed);
        let eff = effect(&mut w, &before, ret);
        let _ = writeln!(out, "{spec}\t{ctx}\t{what}\t{eff}");
    }
    out
}

/// `BO_KEYS25=<file>` writes the shortcut table.
#[test]
#[ignore = "writes the shortcut table (BO_KEYS25=<file>)"]
fn shortcut_inventory() {
    let Ok(path) = std::env::var("BO_KEYS25") else {
        return;
    };
    std::fs::write(path, shortcut_table()).expect("write");
}
