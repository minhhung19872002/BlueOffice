//! Round 23 inventory: every drop-down of every ribbon tab and every context menu, entry by
//! entry, with what running the entry does (a document change, a new window part, a host
//! request, or nothing). `BO_INV23=<file>` writes the table (`cargo test -p bo-ui inventory --
//! --ignored`); the test itself only checks that no entry panics.
use super::super::DocWindow;
use crate::popup::Choice;
use crate::ribbon::Item;
use crate::{DocView, Input, TextMeasure, TextStyle};
use bo_edit::{Pos, Selection};
use bo_engine::edit::Command;
use bo_engine::Engine;
use serde_json::json;
use std::collections::HashSet;
use std::fmt::Write as _;

struct Fixed;
impl TextMeasure for Fixed {
    fn measure(&self, text: &str, style: TextStyle) -> (f32, f32) {
        (
            text.chars().count() as f32 * style.size * 0.55,
            style.size * 1.33,
        )
    }
}

/// Records the texts painted.
#[derive(Default)]
struct Texts(Vec<String>);

impl crate::Painter for Texts {
    fn clear(&mut self, _: bo_render::Rgba) {}
    fn fill_rect(&mut self, _: f32, _: f32, _: f32, _: f32, _: bo_render::Rgba) {}
    fn page(&mut self, _: usize, _: i32, _: i32) {}
    fn text(&mut self, text: &str, _: f32, _: f32, _: TextStyle, _: bo_render::Rgba) {
        self.0.push(text.to_owned());
    }
    fn path(
        &mut self,
        _: &[bo_render::PathSeg],
        _: Option<bo_render::Rgba>,
        _: Option<(bo_render::Rgba, f32)>,
    ) {
    }
    fn push_clip(&mut self, _: f32, _: f32, _: f32, _: f32) {}
    fn pop_clip(&mut self) {}
}

thread_local! {
    /// Round 24: the selection when the window was last painted (a command that only moves
    /// the selection — Next Footnote — has an effect too).
    static LAST_SEL: std::cell::Cell<Option<Selection>> = const { std::cell::Cell::new(None) };
}

fn selection(w: &DocWindow) -> Option<Selection> {
    w.view.engine().session().map(bo_edit::Session::selection)
}

pub(crate) fn painted(w: &mut DocWindow) -> HashSet<String> {
    w.handle(Input::Tick, &Fixed);
    LAST_SEL.with(|c| c.set(selection(w)));
    let mut t = Texts::default();
    w.paint(&mut t);
    t.0.into_iter().collect()
}

/// Where the inventory runs: plain text, a table cell, a picture, a shape, a header.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Ctx {
    Text,
    Table,
    Picture,
    Shape,
    Header,
    /// Round 24: an equation selected (the Equation tab).
    Equation,
}

thread_local! {
    /// Round 24: the second sample — the windows also hold footnotes and endnotes, a list
    /// paragraph (the selection's), a hyperlink, a field, an equation, a comment, a tracked
    /// change, a chart, and the shape has text.
    pub(crate) static RICH: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The second sample's content around the base text (round 24).
fn enrich(w: &mut DocWindow) {
    let caret = |w: &mut DocWindow, p: usize, o: usize| {
        w.view.run_command(Command::SetSelection {
            selection: Selection::caret(Pos::new(p, o)),
        });
    };
    let end = |w: &DocWindow, p: usize| {
        w.view
            .engine()
            .session()
            .and_then(|s| s.para_text(p))
            .map_or(0, |t| t.len())
    };
    // Paragraph 0 ("Hello wrold one two three"): a hyperlink on "two", a footnote and an
    // endnote at its end, bulleted.
    w.view.run_command(Command::SetSelection {
        selection: Selection {
            anchor: Pos::new(0, 16),
            focus: Pos::new(0, 19),
        },
    });
    w.view.run_command(Command::SetLink {
        target: "https://example.com/".into(),
        display: String::new(),
    });
    for endnote in [false, true] {
        let e = end(w, 0);
        caret(w, 0, e);
        w.view.run_command(Command::InsertNote {
            endnote,
            text: "A note".into(),
        });
        w.view.run_command(Command::CloseHeaderFooter);
    }
    // A footnote and an endnote before the selection too (Previous Footnote / Endnote).
    for endnote in [false, true] {
        caret(w, 0, 0);
        w.view.run_command(Command::InsertNote {
            endnote,
            text: "Earlier".into(),
        });
        w.view.run_command(Command::CloseHeaderFooter);
    }
    caret(w, 0, 0);
    if let Some(c) = crate::commands::command("format.bullets") {
        w.view.run_command(c);
    }
    // After paragraph 1: a field, an equation, a commented word, a tracked insertion and a
    // chart, each in a paragraph of its own.
    let e = end(w, 1);
    caret(w, 1, e);
    w.view.run_command(Command::InsertText {
        text: "\nDate: ".into(),
    });
    w.view.run_command(Command::InsertField {
        instr: "DATE".into(),
    });
    w.view.run_command(Command::InsertText {
        text: "\nEquation ".into(),
    });
    let _ = w.r3_run(vec![
        json!({ "cmd": "insertEquation", "text": "a^2+b^2=c^2" }),
    ]);
    w.view.run_command(Command::InsertText {
        text: "\nComment here".into(),
    });
    let p = w
        .view
        .engine()
        .session()
        .map_or(0, |s| s.selection().focus.para);
    w.view.run_command(Command::SetSelection {
        selection: Selection {
            anchor: Pos::new(p, 0),
            focus: Pos::new(p, 7),
        },
    });
    w.view.run_command(Command::AddComment {
        text: "A comment".into(),
        mentions: Vec::new(),
        task: None,
    });
    let e = end(w, p);
    caret(w, p, e);
    w.view.run_command(Command::SetTrackChanges { on: true });
    w.view.run_command(Command::InsertText {
        text: " tracked".into(),
    });
    w.view.run_command(Command::SetTrackChanges { on: false });
    w.view
        .run_command(Command::InsertText { text: "\n".into() });
    let _ = w.r3_run(vec![json!({ "cmd": "insertChart", "kind": "column" })]);
}

/// Select the sample's equation (the Equation tab shows).
fn select_equation(w: &mut DocWindow) {
    let Some(n) = w
        .view
        .engine()
        .session()
        .map(bo_edit::Session::paragraph_count)
    else {
        return;
    };
    for para in 0..n {
        let text = w
            .view
            .engine()
            .session()
            .and_then(|s| s.para_text(para))
            .unwrap_or_default();
        if text.starts_with("Equation ") {
            w.view.run_command(Command::SetSelection {
                selection: Selection {
                    anchor: Pos::new(para, "Equation ".len()),
                    focus: Pos::new(para, text.len()),
                },
            });
            return;
        }
    }
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

pub(crate) fn window(ctx: Ctx) -> DocWindow {
    let mut engine = Engine::with_bundled_fonts();
    engine.new_document("a.docx").expect("new");
    let mut w = DocWindow::new(DocView::new(engine).expect("view"), "en-US", "a.docx");
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1500.0,
            height: 1102.0,
        },
        &Fixed,
    );
    w.view.run_command(Command::InsertText {
        text: "Hello wrold one two three\nSecond paragraph here".into(),
    });
    // Round 24: the second sample; the equation the Equation tab works on.
    if RICH.with(std::cell::Cell::get) {
        enrich(&mut w);
    } else if ctx == Ctx::Equation {
        w.view.run_command(Command::SetSelection {
            selection: Selection::caret(Pos::new(1, 0)),
        });
        w.view.run_command(Command::InsertText {
            text: "Equation \n".into(),
        });
        w.view.run_command(Command::SetSelection {
            selection: Selection::caret(Pos::new(1, "Equation ".len())),
        });
        let _ = w.r3_run(vec![
            json!({ "cmd": "insertEquation", "text": "a^2+b^2=c^2" }),
        ]);
    }
    w.view.run_command(Command::SetSelection {
        selection: Selection {
            anchor: Pos::new(0, 6),
            focus: Pos::new(0, 11),
        },
    });
    match ctx {
        Ctx::Text => {}
        Ctx::Table => {
            w.view.run_command(Command::SetSelection {
                selection: Selection::caret(Pos::new(1, 0)),
            });
            w.view
                .run_command(Command::InsertTable { rows: 2, cols: 2 });
            w.view.run_command(Command::InsertText {
                text: "cell".into(),
            });
        }
        Ctx::Picture => {
            w.view.run_command(Command::SetSelection {
                selection: Selection::caret(Pos::new(1, 0)),
            });
            w.view.insert_picture(png(), "a.png");
            w.view.run_command(Command::Move {
                motion: bo_engine::nav::Motion::Left,
                extend: true,
            });
        }
        Ctx::Shape => {
            // The second sample's shape holds text (a text box).
            w.r3_run(vec![json!({
                "cmd": "insertShape", "preset": "rect", "width": 120.0, "height": 60.0,
                "textBox": RICH.with(std::cell::Cell::get)
            })]);
            // The text box's text is not being edited: the shape is selected.
            if RICH.with(std::cell::Cell::get) {
                w.view.run_command(Command::CloseTextBox);
            }
            w.view.exec(Command::Move {
                motion: bo_engine::nav::Motion::Left,
                extend: true,
            });
        }
        Ctx::Header => {
            let _ = w
                .view
                .run_json(&json!({ "cmd": "editHeaderFooter", "page": 0, "header": true }));
        }
        Ctx::Equation => select_equation(&mut w),
    }
    w.handle(Input::Tick, &Fixed);
    let _ = w.take_requests();
    w.view.mark_saved();
    w
}

/// What running `id` changed: "doc", "ui(+n)", "req", combinations, or "—".
pub(crate) fn effect(w: &mut DocWindow, before: &HashSet<String>, ret: bool) -> String {
    let sel_before = LAST_SEL.with(std::cell::Cell::get);
    let after = painted(w);
    let new = after.difference(before).count();
    let mut out = Vec::new();
    if w.view.modified() {
        out.push("doc".to_owned());
    }
    if sel_before.is_some() && selection(w) != sel_before {
        out.push("sel".to_owned());
    }
    if new > 0 {
        out.push(format!("ui+{new}"));
    }
    let reqs = w.take_requests();
    if !reqs.is_empty() {
        out.push(format!("req:{}", reqs.len()));
    }
    if out.is_empty() {
        if ret {
            "ret".into()
        } else {
            "NONE".into()
        }
    } else {
        out.join(",")
    }
}

/// Rows of the open menu or gallery: (label, command, dim).
pub(crate) fn rows(w: &DocWindow) -> Option<Vec<(String, String, bool)>> {
    if let Some(m) = &w.menu {
        return Some(
            m.items()
                .iter()
                .map(|i| {
                    let c = match &i.choice {
                        Choice::Command(c) => c.clone(),
                        Choice::Font(f) => format!("font:{f}"),
                        Choice::Size(s) => format!("size:{s}"),
                        Choice::Style(s) => format!("style:{s}"),
                    };
                    (i.label.clone(), c, i.dim)
                })
                .collect(),
        );
    }
    if let Some(g) = &w.r22.gallery {
        return Some(
            g.cells()
                .iter()
                .map(|c| (format!("[cell {}] {}", c.0, c.1), c.2.to_owned(), false))
                .collect(),
        );
    }
    None
}

fn ids(tab: &crate::ribbon::Tab) -> Vec<(&'static str, &'static str)> {
    let mut out = Vec::new();
    for g in &tab.groups {
        for it in g.large.iter().chain(g.rows.iter().flatten()) {
            if let Item::Button(id) | Item::Large(id) | Item::Labeled(id) = it {
                out.push((g.label, *id));
            }
        }
    }
    out
}

fn ctx_of(tab: &str) -> Ctx {
    let t = tab.to_ascii_lowercase();
    if t.contains("table") {
        Ctx::Table
    } else if t.contains("picture") {
        Ctx::Picture
    } else if t.contains("shape") || t.contains("drawing") {
        Ctx::Shape
    } else if t.contains("header") {
        Ctx::Header
    } else if t.contains("equation") {
        Ctx::Equation
    } else {
        Ctx::Text
    }
}

/// Window width of the inventory: wide enough that no group collapses.
const WIDE: f32 = 2600.0;

/// A fresh window of `ctx` with tab `tab` shown, after a click on `part` (0: the button's
/// main part, 1: its drop-down part) of ribbon button `id`; `false` when it is not shown.
fn probe(ctx: Ctx, tab: &str, id: &'static str, part: u8) -> (DocWindow, bool, HashSet<String>) {
    let mut w = window(ctx);
    w.handle(
        Input::Resize {
            width: WIDE,
            height: 1102.0,
        },
        &Fixed,
    );
    w.ribbon.select_tab(tab);
    w.relayout(&Fixed);
    let before = painted(&mut w);
    let Some(r) = w.ribbon.rect_of(crate::ribbon::RibbonHit::Command(id)) else {
        let ret = w.action(id);
        return (w, ret, before);
    };
    let large = r[3] > 50.0;
    let (x, y) = match (part, large) {
        (0, true) => (r[0] + r[2] / 2.0, r[1] + r[3] * 0.25),
        (0, false) => (r[0] + 8.0, r[1] + r[3] / 2.0),
        (_, true) => (r[0] + r[2] / 2.0, r[1] + r[3] * 0.85),
        (_, false) => (r[0] + r[2] - 4.0, r[1] + r[3] / 2.0),
    };
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
    (w, true, before)
}

fn dump_menu(
    out: &mut String,
    open: &dyn Fn() -> DocWindow,
    path: &str,
    items: &[(String, String, bool)],
    depth: usize,
) {
    for (label, cmd, dim) in items {
        if (cmd == "ctx.none" || cmd == "ctx.pasteRow") && label.is_empty() {
            continue;
        }
        let runnable = !*dim && cmd != "ctx.none" && (!cmd.contains(':') || cmd.starts_with("r2"));
        let eff = if runnable {
            let mut w = open();
            let before = painted(&mut w);
            let ret = w.action(cmd);
            effect(&mut w, &before, ret)
        } else {
            String::from("-")
        };
        let _ = writeln!(
            out,
            "{path}\t{}{label}\t{cmd}\t{}\t{eff}",
            "  ".repeat(depth),
            if *dim { "greyed" } else { "" }
        );
        // A row that opens a sub-menu.
        if depth == 0 && runnable {
            let mut w = open();
            w.action(cmd);
            if let Some(sub) = rows(&w).filter(|r| r.as_slice() != items) {
                let cmd = cmd.clone();
                let sub_open = move || {
                    let mut w = open();
                    w.action(&cmd);
                    w
                };
                dump_menu(out, &sub_open, path, &sub, depth + 1);
            }
        }
    }
}

fn label(w: &DocWindow, key: &str) -> String {
    w.strings.get(key).to_owned()
}

/// New texts painted (a popup BlueDoc draws itself), shortened.
fn new_texts(w: &mut DocWindow, before: &HashSet<String>) -> String {
    let after = painted(w);
    let mut v: Vec<&String> = after
        .difference(before)
        .filter(|t| !t.trim().is_empty())
        .collect();
    v.sort();
    let s: Vec<&str> = v.iter().take(40).map(|s| s.as_str()).collect();
    s.join(" | ")
}

#[test]
#[ignore = "writes the round-23 inventory (BO_INV23=<file>)"]
fn inventory() {
    let Ok(path) = std::env::var("BO_INV23") else {
        return;
    };
    let only = std::env::var("BO_INV23_TAB").ok();
    std::fs::write(path, build(only.as_deref())).expect("write");
}

/// Entries that do nothing in the inventory's sample windows for a documented reason
/// (round 23 REPORT "Vòng 113"): (command prefix, why).
const ALLOWED: [(&str, &str); 7] = [
    // "None" of the list libraries on a paragraph that is not in a list.
    ("r23.bullet:none", "the sample paragraph has no list"),
    ("r23.number:none", "the sample paragraph has no list"),
    // No footnotes / endnotes in the sample document: nothing to move to.
    ("references.note.", "no notes in the sample"),
    // The sample shape has no text: WordArt styles have no runs to change.
    ("r20.", "the sample shape holds no text"),
    // Equation tab buttons need an equation being edited (the sample has none).
    ("eq.", "no equation in the sample"),
    // Stop Highlighting ends the highlighter pen (greyed on the reference when off).
    ("r23.hl:stop", "highlighter pen is not on"),
    ("ctx.none", "a heading row"),
];

/// Every menu / gallery entry the inventory ran must change something (the document, the
/// window or a host request); the dead ones the baseline found (99) cannot come back.
#[test]
fn no_dead_menu_entries() {
    let out = build(None);
    let dead: Vec<&str> = out
        .lines()
        .filter(|l| l.ends_with("\tNONE") || l.contains("\tNONE\t"))
        .filter(|l| {
            let cmd = l.split('\t').nth(2).unwrap_or("");
            !ALLOWED.iter().any(|(p, _)| cmd.starts_with(p))
        })
        .collect();
    assert!(dead.is_empty(), "dead entries:\n{}", dead.join("\n"));
}

/// Round 24: the second sample (notes, a list paragraph, a hyperlink, a field, an
/// equation, a comment, a tracked change, a chart, a shape with text): only heading rows may
/// do nothing (`BO_INV24=<file>` writes its table).
#[test]
fn no_dead_menu_entries_rich() {
    RICH.with(|r| r.set(true));
    let out = build(None);
    RICH.with(|r| r.set(false));
    if let Ok(path) = std::env::var("BO_INV24") {
        let _ = std::fs::write(path, &out);
    }
    let dead: Vec<&str> = out
        .lines()
        .filter(|l| l.ends_with("\tNONE") || l.contains("\tNONE\t"))
        .filter(|l| {
            let cmd = l.split('\t').nth(2).unwrap_or("");
            !ALLOWED_RICH.iter().any(|(p, _)| cmd.starts_with(p))
        })
        .collect();
    assert!(dead.is_empty(), "dead entries:\n{}", dead.join("\n"));
}

/// What may do nothing in the second sample: (command prefix, why).
const ALLOWED_RICH: [(&str, &str); 1] = [("ctx.none", "a heading row")];

fn build(only: Option<&str>) -> String {
    let mut out = String::from("tab › group › button\tentry\tcommand\tstate\teffect\n");
    let probe_w = window(Ctx::Text);
    let mut tabs = crate::doc_ribbon21::main_tabs();
    tabs.extend(crate::doc_ribbon21::tool_tabs());
    for tab in &tabs {
        let tl = label(&probe_w, tab.label);
        if only.is_some_and(|o| !tl.contains(o)) {
            continue;
        }
        let ctx = ctx_of(&tl);
        let tab_key = tab.label;
        for (group, id) in ids(tab) {
            let path = format!("{tl} › {} › {}", label(&probe_w, group), {
                let k = format!("commands.{id}");
                let l = label(&probe_w, &k);
                if l == k {
                    id.to_owned()
                } else {
                    l
                }
            });
            let mut seen: Option<Vec<(String, String, bool)>> = None;
            for part in [0u8, 1] {
                let (mut w, ret, before) = probe(ctx, tab_key, id, part);
                let what = if part == 0 { "click" } else { "▾" };
                if let Some(r) = rows(&w) {
                    if seen.as_ref() == Some(&r) {
                        continue;
                    }
                    let _ = writeln!(out, "{path}\t({what}: menu {} rows)\t{id}\t\t", r.len());
                    let open = move || probe(ctx, tab_key, id, part).0;
                    dump_menu(&mut out, &open, &path, &r, 0);
                    seen = Some(r);
                } else {
                    let texts = new_texts(&mut w, &before);
                    let eff = effect(&mut w, &before, ret);
                    if part == 1 && seen.is_none() && eff.starts_with("ui") {
                        // The drop-down part shows its own popup (not a menu BlueDoc lists).
                        let _ = writeln!(out, "{path}\t({what}: popup)\t{id}\t\t{eff}\t{texts}");
                    } else if part == 0 {
                        let _ = writeln!(out, "{path}\t({what})\t{id}\t\t{eff}\t{texts}");
                    }
                }
            }
        }
    }
    // Context menus.
    for (name, ctx) in [
        ("context: text", Ctx::Text),
        ("context: table cell", Ctx::Table),
        ("context: picture", Ctx::Picture),
        ("context: shape", Ctx::Shape),
        ("context: header", Ctx::Header),
    ] {
        if only.is_some() {
            break;
        }
        let w = window(ctx);
        let (x, y) = context_point(&w, ctx);
        let open = move || {
            let mut w = window(ctx);
            w.handle(Input::Context { x, y }, &Fixed);
            w
        };
        let w = open();
        let Some(r) = rows(&w) else {
            let _ = writeln!(out, "{name}\t(no menu)\t\t\t");
            continue;
        };
        let _ = writeln!(out, "{name}\t(menu {} rows)\t\t\t", r.len());
        dump_menu(&mut out, &open, name, &r, 0);
    }
    out
}
/// A window point over the context's object (the selection for text).
pub(crate) fn context_point(w: &DocWindow, ctx: Ctx) -> (f32, f32) {
    let layout = w.view.engine().layout().expect("layout");
    let slot = w.view.slots()[0];
    let s = w.view.scale();
    let page = &layout.pages[slot.index];
    let caret = w.view.engine().session().map(|s| s.selection().focus);
    let para = match ctx {
        Ctx::Text => 0,
        _ => caret.map_or(0, |p| p.para),
    };
    let line = page
        .lines
        .iter()
        .find(|l| l.source == bo_layout::LineSource::Body(para as u32))
        .or_else(|| page.lines.first());
    let Some(line) = line else {
        return (slot.x as f32 + 100.0, slot.y as f32 + 100.0);
    };
    let x = line.carets.get(8).map_or(10.0, |c| c.1);
    (
        slot.x as f32 + x * s,
        slot.y as f32 + (line.top + line.height / 2.0) * s,
    )
}
