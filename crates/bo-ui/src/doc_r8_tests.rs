//! BlueDoc parity round 8: editing keys, mouse gestures and AutoCorrect as on the reference.
use super::super::DocWindow;
use crate::{DocView, Input, Key, Mods, TextMeasure, TextStyle};
use bo_edit::{Pos, Selection};
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
    text.char_indices().nth(chars).map_or(text.len(), |(i, _)| i)
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
