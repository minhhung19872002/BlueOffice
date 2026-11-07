// Shared by `benches/doc_typing.rs` and `tests/typing_perf.rs`: the ~300-page
// benchmark document of BlueOffice-PROMPT §5 ("gõ phím → hiển thị < 16 ms ở tài liệu 300 trang").
#![allow(dead_code)]

use bo_engine::{Command, Engine};
use bo_model::{
    Alignment, Block, Document, HalfPoints, ParaProps, Paragraph, Run, RunProps, Section, Twips,
};

const TEXT: &str = "Căn cứ Luật Tổ chức Chính phủ ngày 19 tháng 6 năm 2015; Căn cứ Luật sửa đổi, bổ sung một số điều của Luật Tổ chức Chính phủ và Luật Tổ chức chính quyền địa phương ngày 22 tháng 11 năm 2019; Theo đề nghị của Bộ trưởng Bộ Nội vụ.";

/// Paragraphs of the benchmark document (about 300 A4 pages).
pub const PARAGRAPHS: usize = 4300;

/// Justified Vietnamese text, Times New Roman 13 pt, `paragraphs` paragraphs.
pub fn big_doc(paragraphs: usize) -> Document {
    let blocks = (0..paragraphs)
        .map(|i| {
            Block::Paragraph(Paragraph::new(
                ParaProps {
                    align: Some(Alignment::Justify),
                    indent_first_line: Some(Twips(567)),
                    space_after: Some(Twips(120)),
                    ..Default::default()
                },
                vec![Run::text(format!("{i}. {TEXT}"), RunProps::default())],
            ))
        })
        .collect();
    Document {
        default_run: RunProps {
            font: Some("Times New Roman".into()),
            size: Some(HalfPoints(26)),
            ..Default::default()
        },
        sections: vec![Section::new(blocks)],
        ..Default::default()
    }
}

/// Engine with the benchmark document open and the caret in the middle paragraph.
pub fn engine_with_big_doc() -> Engine {
    let mut e = Engine::with_bundled_fonts();
    e.load(big_doc(PARAGRAPHS), "bench.docx").expect("load");
    caret_at(&mut e, PARAGRAPHS / 2, 10);
    e
}

/// Put a selection from (`para`, `start`) to (`para`, `end`).
pub fn select(e: &mut Engine, para: usize, start: usize, end: usize) {
    let selection = bo_edit::Selection {
        anchor: bo_edit::Pos::new(para, start),
        focus: bo_edit::Pos::new(para, end),
    };
    e.command(Command::SetSelection { selection })
        .expect("selection");
}

/// Put a collapsed selection at (`para`, `offset`).
pub fn caret_at(e: &mut Engine, para: usize, offset: usize) {
    select(e, para, offset, offset);
}

/// Run `cmd`, then fetch the display list of the page showing the caret (what the
/// canvas repaints first). Returns the length of that page's JSON.
pub fn edit_and_fetch_visible(e: &mut Engine, cmd: Command) -> usize {
    let r = e.command(cmd).expect("command");
    let page = r.caret.as_ref().map_or(0, |c| c.page);
    e.page_json(page).expect("page").len()
}

/// The four timed edits of the §5 target, each followed by its untimed inverse so the
/// document stays the same: (name, timed command, restoring command).
pub fn edits() -> Vec<(&'static str, Command, Command)> {
    vec![
        (
            "type_char",
            Command::InsertText { text: "a".into() },
            Command::DeleteBackward,
        ),
        ("enter", Command::SplitParagraph, Command::DeleteBackward),
        // An empty paragraph after the middle one: every later page break moves.
        (
            "enter_at_paragraph_end",
            Command::SplitParagraph,
            Command::DeleteBackward,
        ),
        (
            WORST_CASE,
            Command::InsertText { text: "a".into() },
            Command::DeleteBackward,
        ),
        (
            "backspace",
            Command::DeleteBackward,
            Command::InsertText { text: "C".into() },
        ),
        (
            "bold_toggle",
            Command::FormatText {
                format: bo_edit::CharFormat::Bold,
            },
            Command::FormatText {
                format: bo_edit::CharFormat::Bold,
            },
        ),
    ]
}

/// Typing when no page break re-synchronises: every page after the edit is laid out again.
pub const WORST_CASE: &str = "type_char_worst_case";

/// Prepare the selection for edit `name` in the middle paragraph.
pub fn prepare(e: &mut Engine, name: &str) {
    let mid = PARAGRAPHS / 2;
    e.set_layout_resync(name != WORST_CASE);
    match name {
        // Select the word "Luật" ("2150. Căn cứ Luật": byte offsets 16..22).
        "bold_toggle" => select(e, mid, 16, 22),
        // After "2150. C" so backspace removes "C" and the restore types it back.
        "backspace" => caret_at(e, mid, 7),
        "enter_at_paragraph_end" => caret_at(e, mid, usize::MAX),
        _ => caret_at(e, mid, 10),
    }
}
