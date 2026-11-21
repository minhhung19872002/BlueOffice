//! BlueDoc parity round 8: editing keys, mouse gestures and AutoCorrect as on the reference.
use super::super::DocWindow;
use crate::{DocView, Input, Key, Mods, TextMeasure, TextStyle};
use bo_edit::{ParaFormat, Pos, Selection};
use bo_engine::edit::Command;
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
    w.handle(Input::Scale(1.25), &Fixed);
    w.handle(
        Input::Resize {
            width: 1500.0,
            height: 1102.0,
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

fn key(w: &mut DocWindow, k: Key, mods: Mods) {
    w.handle(Input::Key(k, mods), &Fixed);
}

#[test]
fn extend_mode_selects_a_word_and_escape_restores_normal_navigation() {
    let mut w = window();
    w.view.run_command(Command::InsertText {
        text: "alpha beta".into(),
    });
    w.view.run_command(Command::SetSelection {
        selection: Selection::caret(Pos::new(0, 1)),
    });
    key(&mut w, Key::F(8), Mods::default());
    key(&mut w, Key::F(8), Mods::default());
    let s = w
        .view
        .engine()
        .session()
        .expect("session")
        .selection()
        .ordered();
    assert_eq!(s, (Pos::new(0, 0), Pos::new(0, 6)));
    key(&mut w, Key::Escape, Mods::default());
    key(&mut w, Key::Right, Mods::default());
    assert!(w
        .view
        .engine()
        .session()
        .expect("session")
        .selection()
        .is_collapsed());
}

#[test]
fn formatting_shortcuts_apply_to_selection_and_undo() {
    let mut w = window();
    w.view.run_command(Command::InsertText {
        text: "alpha".into(),
    });
    w.view.run_command(Command::SetSelection {
        selection: Selection {
            anchor: Pos::new(0, 0),
            focus: Pos::new(0, 5),
        },
    });
    key(
        &mut w,
        Key::Letter('b'),
        Mods {
            ctrl: true,
            ..Mods::default()
        },
    );
    assert!(w.view.format().bold);
    w.view.run_command(Command::Undo);
    assert!(!w.view.format().bold);
}

#[test]
fn autocorrect_runs_for_typed_characters_and_undo_reverts_the_correction() {
    let mut w = window();
    for c in "(c)".chars() {
        w.handle(Input::Text(c.to_string()), &Fixed);
    }
    assert_eq!(para_text(&w, 0), "©");
    w.view.run_command(Command::Undo);
    assert_eq!(para_text(&w, 0), "(c)");
}

#[test]
fn autocorrect_does_not_override_existing_host_settings() {
    let mut engine = Engine::with_bundled_fonts();
    engine.new_document("a.docx").expect("new");
    let options = bo_edit::AutoCorrectOptions {
        enabled: false,
        ..bo_edit::AutoCorrectOptions::default()
    };
    engine
        .command(Command::AutoCorrect {
            options: Some(Box::new(options)),
            reset: false,
        })
        .expect("configure");
    let mut w = window_with(engine);
    for c in "(c)".chars() {
        w.handle(Input::Text(c.to_string()), &Fixed);
    }
    assert_eq!(para_text(&w, 0), "(c)");
    assert!(
        !w.view
            .engine()
            .autocorrect_options()
            .expect("configured")
            .enabled
    );
}

/// Parse a key spec of the comparison scripts ("C+S+Right", "F8", "t:typed_text").
pub(super) fn spec_inputs(spec: &str) -> Vec<Input> {
    if let Some(t) = spec.strip_prefix("t:") {
        return t
            .replace('_', " ")
            .chars()
            .map(|c| Input::Text(c.to_string()))
            .collect();
    }
    let parts: Vec<&str> = spec.split('+').collect();
    let mut m = Mods::default();
    let name = if spec.ends_with("++") || spec == "+" {
        "+"
    } else {
        parts.last().copied().unwrap_or("")
    };
    for p in &parts[..parts.len().saturating_sub(1)] {
        match *p {
            "C" => m.ctrl = true,
            "S" => m.shift = true,
            "A" => m.alt = true,
            _ => {}
        }
    }
    let key = match name {
        "Left" => Key::Left,
        "Right" => Key::Right,
        "Up" => Key::Up,
        "Down" => Key::Down,
        "Home" => Key::Home,
        "End" => Key::End,
        "PgUp" => Key::PageUp,
        "PgDn" => Key::PageDown,
        "Back" => Key::Backspace,
        "Del" => Key::Delete,
        "Enter" => Key::Enter,
        "Tab" => Key::Tab,
        "Esc" => Key::Escape,
        "Space" => Key::Letter(' '),
        f if f.len() > 1 && f.starts_with('F') => Key::F(f[1..].parse().unwrap_or(0)),
        // Ctrl+= reaches the window as '+' (win32 `key`), with or without Shift.
        "=" => Key::Letter('+'),
        s => Key::Letter(s.chars().next().unwrap_or(' ').to_ascii_lowercase()),
    };
    if name == "Space" && !m.ctrl {
        return vec![Input::Text(" ".into())];
    }
    vec![Input::Key(key, m)]
}

/// Word paragraph number (1-based, cells and row ends counted) of a body paragraph of the
/// comparison file `k-src.docx` and back.
fn to_ours(p: usize) -> usize {
    match p {
        0..=7 => p - 1,
        9 | 10 => p - 2,
        _ => p - 3,
    }
}

fn to_word(p: usize) -> usize {
    match p {
        0..=6 => p + 1,
        7 | 8 => p + 2,
        _ => p + 3,
    }
}

fn byte_of(text: &str, chars: usize) -> usize {
    text.char_indices()
        .nth(chars)
        .map_or(text.len(), |(i, _)| i)
}

fn chars_of(text: &str, bytes: usize) -> usize {
    text.get(..bytes.min(text.len()))
        .map_or(0, |t| t.chars().count())
}

fn para_text(w: &DocWindow, p: usize) -> String {
    w.view
        .engine()
        .session()
        .and_then(|s| s.para_text(p))
        .unwrap_or_default()
}

pub(super) fn open_sample(bytes: &[u8]) -> DocWindow {
    let mut engine = Engine::with_bundled_fonts();
    engine.open(bytes, "k-src.docx").expect("open");
    window_with(engine)
}

pub(super) fn select(w: &mut DocWindow, a: (usize, usize), b: (usize, usize)) {
    let (pa, pb) = (to_ours(a.0), to_ours(b.0));
    let (ta, tb) = (para_text(w, pa), para_text(w, pb));
    let selection = Selection {
        anchor: Pos {
            para: pa,
            offset: byte_of(&ta, a.1),
        },
        focus: Pos {
            para: pb,
            offset: byte_of(&tb, b.1),
        },
    };
    w.view.run_command(Command::SetSelection { selection });
}

fn describe(w: &DocWindow) -> Vec<String> {
    let s = w.view.engine().session().expect("session");
    let sel = s.selection();
    let (a, b) = sel.ordered();
    let n = s.paragraph_count();
    let pos = |p: Pos| {
        let t = para_text(w, p.para);
        format!("{},{}", to_word(p.para), chars_of(&t, p.offset))
    };
    let mut selected = String::new();
    for p in a.para..=b.para {
        let t = para_text(w, p);
        let from = if p == a.para { a.offset } else { 0 };
        let to = if p == b.para { b.offset } else { t.len() };
        selected.push_str(t.get(from.min(t.len())..to.min(t.len())).unwrap_or(""));
        if p != b.para {
            selected.push('¶');
        }
    }
    let f = w.view.format();
    let info = format!(
        "list={} b={} i={} u={} sz={} al={:?} ls={:?}/{} sb={} sty={} ind={} fi={} sub={} sup={} pars={}",
        u8::from(f.bullets || f.numbering),
        u8::from(f.bold),
        u8::from(f.italic),
        u8::from(f.underline),
        f.size_half_points.map_or(0.0, |h| h as f32 / 2.0),
        f.align,
        f.line_spacing.rule,
        f.line_spacing.value,
        f.space_before,
        f.style.clone().unwrap_or_default(),
        f.indent_left,
        f.first_line,
        u8::from(f.subscript),
        u8::from(f.superscript),
        n,
    );
    let text: Vec<String> = (0..n)
        .map(|p| para_text(w, p).replace('\t', "→").replace('\u{b}', "↵"))
        .collect();
    vec![pos(a), pos(b), selected, info, text.join("¶")]
}

/// Our side of the key behaviour table: `BO_KEYS=<scenarios.txt> BO_KEYS_SRC=<k-src.docx>
/// BO_KEYS_OUT=<out.tsv> cargo test -p bo-ui key_table -- --ignored`.
#[test]
#[ignore = "writes the key behaviour table for the comparison with the reference"]
fn key_table() {
    let (Ok(scen), Ok(src), Ok(out)) = (
        std::env::var("BO_KEYS"),
        std::env::var("BO_KEYS_SRC"),
        std::env::var("BO_KEYS_OUT"),
    ) else {
        return;
    };
    let bytes = std::fs::read(src).expect("src");
    let mut rows = Vec::new();
    for line in std::fs::read_to_string(scen).expect("scen").lines() {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('|').collect();
        let n = |i: usize| f[i].parse::<usize>().unwrap_or(0);
        let mut w = open_sample(&bytes);
        select(&mut w, (n(1), n(2)), (n(3), n(4)));
        for k in f[5].split(' ').filter(|k| !k.is_empty()) {
            for i in spec_inputs(k) {
                w.handle(i, &Fixed);
            }
        }
        let mut row = vec![f[0].to_owned()];
        row.extend(describe(&w));
        rows.push(row.join("\t"));
    }
    std::fs::write(out, rows.join("\n")).expect("out");
}

// ------------------------------------------------------------------ mouse (wmouse.ps1)

const MOUSE_TEXT: &str = "The quick brown fox jumps over the lazy dog. It's well-known; isn't it? Yes.\nThird paragraph with some words to move through and a long enough line so that it wraps onto a second line in the page at the default margins for sure.\nLast paragraph.";

fn mouse_window() -> DocWindow {
    let mut w = window();
    w.view.run_command(Command::InsertText {
        text: MOUSE_TEXT.into(),
    });
    w.view.run_command(Command::SetSelection {
        selection: Selection::caret(Pos::new(0, 0)),
    });
    w
}

/// Window pixel of byte `offset` of paragraph `para` (on its line, vertically centred), and
/// the line's (start, end) bytes.
fn pixel_of(w: &DocWindow, para: usize, offset: usize) -> (f32, f32, (usize, usize)) {
    let layout = w.view.engine().layout().expect("layout");
    let slot = w.view.slots()[0];
    let s = w.view.scale();
    let page = &layout.pages[slot.index];
    let line = page
        .lines
        .iter()
        .find(|l| {
            l.source == bo_layout::LineSource::Body(para as u32)
                && (l.start as usize) <= offset
                && offset < (l.end as usize).max(l.start as usize + 1)
        })
        .expect("line");
    let x = line
        .carets
        .iter()
        .find(|c| c.0 as usize >= offset)
        .map_or(0.0, |c| c.1);
    (
        slot.x as f32 + (x + 2.0) * s,
        slot.y as f32 + (line.top + line.height / 2.0) * s,
        (line.start as usize, line.end as usize),
    )
}

fn margin_x(w: &DocWindow) -> f32 {
    let slot = w.view.slots()[0];
    slot.x as f32 + 30.0 * w.view.scale()
}

fn press(w: &mut DocWindow, x: f32, y: f32, count: u8, shift: bool) {
    w.handle(Input::Press { x, y, count, shift }, &Fixed);
    w.handle(Input::Release { x, y }, &Fixed);
}

fn sel(w: &DocWindow) -> (Pos, Pos) {
    w.view
        .engine()
        .session()
        .expect("session")
        .selection()
        .ordered()
}

#[test]
fn double_click_selects_the_word_with_its_space_and_triple_the_paragraph() {
    let mut w = mouse_window();
    let (x, y, _) = pixel_of(&w, 0, 6);
    press(&mut w, x, y, 1, false);
    press(&mut w, x, y, 2, false);
    // Reference: "quick " (dbl_word).
    assert_eq!(sel(&w), (Pos::new(0, 4), Pos::new(0, 10)));
    let (x, y, _) = pixel_of(&w, 1, 20);
    press(&mut w, x, y, 3, false);
    // Reference: the paragraph with its mark (tpl_para).
    assert_eq!(sel(&w), (Pos::new(1, 0), Pos::new(2, 0)));
}

#[test]
fn ctrl_click_selects_the_sentence_with_the_spaces_after_it() {
    let mut w = mouse_window();
    let (x, y, _) = pixel_of(&w, 0, 50);
    w.set_pointer_mods(Mods {
        ctrl: true,
        ..Mods::default()
    });
    press(&mut w, x, y, 1, false);
    // Reference: "It's well-known; isn't it? " (ctrl_sentence).
    let (a, b) = sel(&w);
    let t = para_text(&w, 0);
    assert_eq!(&t[a.offset..b.offset], "It's well-known; isn't it? ");
    // The Ctrl state is used once: the next plain click puts the caret.
    press(&mut w, x, y, 1, false);
    assert!(w
        .view
        .engine()
        .session()
        .expect("session")
        .selection()
        .is_collapsed());
}

#[test]
fn shift_click_extends_from_the_anchor() {
    let mut w = mouse_window();
    w.view.run_command(Command::SetSelection {
        selection: Selection::caret(Pos::new(0, 30)),
    });
    let (x, y, _) = pixel_of(&w, 0, 6);
    press(&mut w, x, y, 1, true);
    let s = w.view.engine().session().expect("session").selection();
    assert_eq!(s.anchor, Pos::new(0, 30));
    assert!(s.focus.offset <= 7 && s.focus.offset >= 5, "{s:?}");
}

#[test]
fn selection_bar_selects_line_paragraph_and_document() {
    let mut w = mouse_window();
    let (_, y1, (s1, e1)) = pixel_of(&w, 1, 0);
    assert!(e1 < para_text(&w, 1).len(), "the paragraph wraps");
    let x = margin_x(&w);
    // One click: the first line with its trailing space (margin_line: 0..88 on the reference).
    press(&mut w, x, y1, 1, false);
    assert_eq!(sel(&w), (Pos::new(1, s1), Pos::new(1, e1)));
    assert!(para_text(&w, 1)[..e1].ends_with(' '));
    // The last line: up to the next paragraph (its mark selected, margin_line_last).
    let len = para_text(&w, 1).len();
    let (_, y2, _) = pixel_of(&w, 1, len - 3);
    press(&mut w, x, y2, 1, false);
    assert_eq!(sel(&w).1, Pos::new(2, 0));
    // Double click: the paragraph (margin_dbl).
    press(&mut w, x, y1, 1, false);
    press(&mut w, x, y1, 2, false);
    assert_eq!(sel(&w), (Pos::new(1, 0), Pos::new(2, 0)));
    // Triple click and Ctrl+click: the whole document (margin_tpl, margin_ctrl).
    press(&mut w, x, y1, 3, false);
    let last = para_text(&w, 2).len();
    assert_eq!(sel(&w), (Pos::new(0, 0), Pos::new(2, last)));
    press(&mut w, x, y1, 1, false);
    w.set_pointer_mods(Mods {
        ctrl: true,
        ..Mods::default()
    });
    press(&mut w, x, y1, 1, false);
    assert_eq!(sel(&w), (Pos::new(0, 0), Pos::new(2, last)));
}

#[test]
fn drag_snaps_to_whole_words_after_the_first_word() {
    let mut w = mouse_window();
    let (x0, y, _) = pixel_of(&w, 0, 6);
    w.handle(
        Input::Press {
            x: x0,
            y,
            count: 1,
            shift: false,
        },
        &Fixed,
    );
    let (x1, _, _) = pixel_of(&w, 0, 17);
    w.handle(Input::Drag { x: x1, y }, &Fixed);
    w.handle(Input::Release { x: x1, y }, &Fixed);
    // From inside "quick" to inside "fox": both words whole.
    let (a, b) = sel(&w);
    assert_eq!(a, Pos::new(0, 4));
    assert!(b.offset >= 19, "{b:?}");
}

#[test]
fn typing_over_a_triple_clicked_paragraph_keeps_its_mark() {
    let mut w = mouse_window();
    let (x, y, _) = pixel_of(&w, 1, 20);
    press(&mut w, x, y, 3, false);
    w.handle(Input::Text("X".into()), &Fixed);
    assert_eq!(para_text(&w, 1), "X");
    assert_eq!(para_text(&w, 2), "Last paragraph.");
}

#[test]
fn typing_is_one_undo_step_until_a_correction_like_the_reference() {
    // undo_typing / undo_typing2 of the key table: " one two three" after "Last paragraph."
    // undoes "two three" first (the capital of "One" split the run), then the capital.
    let mut w = window();
    w.view.run_command(Command::InsertText {
        text: "Last paragraph.".into(),
    });
    for c in " one two three".chars() {
        w.handle(Input::Text(c.to_string()), &Fixed);
    }
    assert_eq!(para_text(&w, 0), "Last paragraph. One two three");
    w.view.run_command(Command::Undo);
    assert_eq!(para_text(&w, 0), "Last paragraph. One ");
    w.view.run_command(Command::Undo);
    assert_eq!(para_text(&w, 0), "Last paragraph. one ");
}

#[test]
fn undo_list_names_the_steps_and_undoes_several() {
    let mut w = window();
    for c in "abc def".chars() {
        w.handle(Input::Text(c.to_string()), &Fixed);
    }
    w.view.run_command(Command::SelectAll);
    key(
        &mut w,
        Key::Letter('b'),
        Mods {
            ctrl: true,
            ..Mods::default()
        },
    );
    let labels = w.r8_undo_labels();
    assert_eq!(labels[0], "Bold");
    // "abc " was capitalised at the space (its own step), then "def" typed.
    assert_eq!(labels[1], "Typing \"def\"", "{labels:?}");
    assert_eq!(labels[2], "AutoCorrect", "{labels:?}");
    assert_eq!(labels[3], "Typing \"abc \"", "{labels:?}");
    // The arrow sits right of Undo in the title bar and opens the list.
    w.handle(Input::Tick, &Fixed);
    let r = w
        .title
        .quick_rect(crate::titlebar::UNDO_LIST)
        .expect("undo list arrow");
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
    assert!(w.menu.is_some());
    w.menu = None;
    // Choosing the second row undoes it and the newer step.
    assert_eq!(w.r8_action("edit.undoSteps:2"), Some(true));
    assert_eq!(para_text(&w, 0), "Abc ");
    assert_eq!(w.r8_action("edit.undoSteps:2"), Some(true));
    assert_eq!(para_text(&w, 0), "");
}

#[test]
fn autocorrect_dialog_opens_from_options_and_its_choices_reach_the_engine() {
    let mut w = window();
    assert_eq!(w.r8_action("tools.autoCorrect"), Some(true));
    assert!(w.r8.autocorrect.is_some());
    // Escape closes it without change.
    key(&mut w, Key::Escape, Mods::default());
    assert!(w.r8.autocorrect.is_none());
    w.r8_open_autocorrect();
    let opts = bo_edit::AutoCorrectOptions {
        replace_text: false,
        ..bo_edit::AutoCorrectOptions::default()
    };
    w.view.run_command(Command::AutoCorrect {
        options: Some(Box::new(opts)),
        reset: false,
    });
    w.r8.autocorrect = None;
    for c in "x (c) ".chars() {
        w.handle(Input::Text(c.to_string()), &Fixed);
    }
    assert_eq!(para_text(&w, 0), "X (c) ");
}

#[test]
fn enter_ends_an_empty_list_item_and_ctrl_enter_puts_the_break_in_its_own_paragraph() {
    let mut w = window();
    w.view.run_command(Command::InsertText {
        text: "item".into(),
    });
    w.view.run_command(Command::FormatParagraph {
        format: ParaFormat::Bullets,
    });
    key(&mut w, Key::Enter, Mods::default());
    assert!(w.view.format().bullets);
    // enter_list_empty: the second Enter ends the list without a new paragraph.
    key(&mut w, Key::Enter, Mods::default());
    assert!(!w.view.format().bullets);
    assert_eq!(w.view.engine().session().expect("s").paragraph_count(), 2);
    // center: "ab|cd" + Ctrl+Enter → "ab", page break, "cd".
    let mut w = window();
    w.view.run_command(Command::InsertText {
        text: "abcd".into(),
    });
    w.view.run_command(Command::SetSelection {
        selection: Selection::caret(Pos::new(0, 2)),
    });
    key(
        &mut w,
        Key::Enter,
        Mods {
            ctrl: true,
            ..Mods::default()
        },
    );
    // The capital comes from AutoCorrect at the break, as typing would.
    assert_eq!(para_text(&w, 0), "Ab");
    assert_eq!(para_text(&w, 2), "cd");
    assert_eq!(sel(&w).0, Pos::new(2, 0));
}

#[test]
fn three_hyphens_and_enter_become_a_bottom_border() {
    let mut w = window();
    for c in "---".chars() {
        w.handle(Input::Text(c.to_string()), &Fixed);
    }
    key(&mut w, Key::Enter, Mods::default());
    assert_eq!(para_text(&w, 0), "");
    assert_eq!(sel(&w).0, Pos::new(1, 0));
    let doc = w.view.engine().session().expect("s").doc();
    let bottom = |i: usize| {
        bo_model::index::para_by_id(doc, i)
            .and_then(|p| p.props.borders.as_ref())
            .and_then(|b| b.bottom.as_ref())
            .is_some()
    };
    assert!(bottom(0) && !bottom(1));
}

#[test]
fn three_hyphens_after_a_paragraph_underline_that_paragraph() {
    let mut w = window();
    w.view.run_command(Command::InsertText {
        text: "Last paragraph.
"
        .into(),
    });
    for c in "---".chars() {
        w.handle(Input::Text(c.to_string()), &Fixed);
    }
    key(&mut w, Key::Enter, Mods::default());
    assert_eq!(w.view.engine().session().expect("s").paragraph_count(), 2);
    assert_eq!(para_text(&w, 1), "");
    assert_eq!(sel(&w).0, Pos::new(1, 0));
    let doc = w.view.engine().session().expect("s").doc();
    let bottom = |i: usize| {
        bo_model::index::para_by_id(doc, i)
            .and_then(|p| p.props.borders.as_ref())
            .and_then(|b| b.bottom.as_ref())
            .is_some()
    };
    assert!(bottom(0) && !bottom(1));
}
