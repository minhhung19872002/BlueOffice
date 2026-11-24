//! Command ids (the same ids as the React `CommandRegistry`, e.g. `format.bold`) mapped to
//! engine commands, with their toggle and enabled state from the engine's format state.
use bo_edit::{CharFormat, FormatState, ParaFormat};
use bo_engine::edit::Command;
use bo_model::Alignment;

/// The engine command run by command `id`, if the native interface implements it.
pub fn command(id: &str) -> Option<Command> {
    let text = |format| Some(Command::FormatText { format });
    let para = |format| Some(Command::FormatParagraph { format });
    let align = |align| para(ParaFormat::Align { align });
    match id {
        "edit.undo" => Some(Command::Undo),
        "edit.redo" => Some(Command::Redo),
        "format.bold" => text(CharFormat::Bold),
        "format.italic" => text(CharFormat::Italic),
        "format.underline" => text(CharFormat::Underline),
        "format.strike" => text(CharFormat::Strike),
        "format.subscript" => text(CharFormat::Subscript),
        "format.superscript" => text(CharFormat::Superscript),
        "format.alignLeft" => align(Alignment::Left),
        "format.alignCenter" => align(Alignment::Center),
        "format.alignRight" => align(Alignment::Right),
        "format.justify" => align(Alignment::Justify),
        "format.bullets" => para(ParaFormat::Bullets),
        "format.numbering" => para(ParaFormat::Numbering),
        "format.indent" => para(ParaFormat::IncreaseIndent),
        "format.outdent" => para(ParaFormat::DecreaseIndent),
        // Split buttons apply their default colour (red text, yellow highlight).
        "format.color" => text(CharFormat::Color {
            hex: Some("FF0000".into()),
        }),
        "format.highlight" => text(CharFormat::Highlight {
            hex: Some("FFFF00".into()),
        }),
        _ => None,
    }
}

/// Interface name key of a built-in style (the document stores English names).
pub fn style_name_key(name: &str) -> Option<&'static str> {
    Some(match name.to_ascii_lowercase().as_str() {
        "normal" => "commands.format.styleNormal",
        "title" => "commands.format.styleTitle",
        "heading 1" => "commands.format.styleHeading1",
        "heading 2" => "commands.format.styleHeading2",
        "heading 3" => "commands.format.styleHeading3",
        _ => return None,
    })
}

/// The `'static` id of a ribbon command (menus anchor to the button that opened them).
pub fn static_id(id: &str) -> &'static str {
    const MENUS: [&str; 23] = [
        "insert.shapes",
        "insert.pageNumber",
        "insert.dropCap",
        "layout.lineNumbers",
        "layout.hyphenation",
        "references.insertCaption",
        "format.textEffects",
        "format.multilevel",
        "format.changeCase",
        "format.shading",
        "format.paraBorders",
        "format.sortParagraphs",
        "layout.margins",
        "layout.orientation",
        "layout.size",
        "layout.columns",
        "layout.breaks",
        "insert.table",
        "insert.symbol",
        "design.pageColor",
        "design.watermark",
        "view.layout",
        "format.lineSpacing",
    ];
    MENUS
        .iter()
        .find(|m| **m == id)
        .copied()
        .or_else(|| crate::doc_tools::static_id(id))
        .or_else(|| crate::doc_r3_menus::static_id(id))
        .or_else(|| crate::doc_r4_menus::static_id(id))
        .or_else(|| crate::doc_r4_fit::static_id(id))
        .or_else(|| crate::doc_review16::static_id(id))
        .unwrap_or("")
}

/// Commands the window handles itself (dialogs, view state, format painter, mail merge).
pub const WINDOW: [&str; 34] = [
    // Round 22: Home › Add-ins (File › Options › Add-ins).
    "home.addins",
    "view.readMode",
    "layout.pageSetup",
    "view.printLayout",
    "view.webLayout",
    "view.draft",
    "view.onePage",
    "view.pageWidth",
    "review.spellingDialog",
    "insert.picture",
    "view.navigationPane",
    "format.fontSettings",
    "format.paragraphSettings",
    "review.deleteComment",
    "edit.find",
    "edit.replace",
    "insert.link",
    "review.newComment",
    "review.wordCount",
    "review.trackChanges",
    "view.ruler",
    "view.formattingMarks",
    "view.zoomIn",
    "view.zoomOut",
    "view.zoomReset",
    "format.painter",
    "format.growStep",
    "format.shrinkStep",
    "mailings.recipients",
    "mailings.preview",
    "mailings.previousRecord",
    "mailings.nextRecord",
    "mailings.finish",
    // Round 12: Home › Borders ▾ › Borders and Shading….
    "format.bordersShading",
];

/// Keyboard shortcut shown in the tooltip of command `id`.
pub fn shortcut(id: &str) -> Option<&'static str> {
    Some(match id {
        "edit.undo" => "Ctrl+Z",
        "edit.redo" => "Ctrl+Y",
        "edit.cut" => "Ctrl+X",
        "edit.copy" => "Ctrl+C",
        "edit.paste" => "Ctrl+V",
        "file.save" => "Ctrl+S",
        "edit.find" => "Ctrl+F",
        "edit.replace" => "Ctrl+H",
        "insert.link" => "Ctrl+K",
        "format.painter" => "Ctrl+Shift+C",
        "format.bold" => "Ctrl+B",
        "format.italic" => "Ctrl+I",
        "format.underline" => "Ctrl+U",
        "format.subscript" => "Ctrl+=",
        "format.superscript" => "Ctrl+Shift++",
        "format.alignLeft" => "Ctrl+L",
        "format.alignCenter" => "Ctrl+E",
        "format.alignRight" => "Ctrl+R",
        "format.justify" => "Ctrl+J",
        _ => return None,
    })
}

/// Whether command `id` shows as switched on for the selection.
pub fn toggled(id: &str, f: &FormatState) -> bool {
    match id {
        "format.bold" => f.bold,
        "format.italic" => f.italic,
        "format.underline" => f.underline,
        "format.strike" => f.strike,
        "format.subscript" => f.subscript,
        "format.superscript" => f.superscript,
        "format.alignLeft" => f.align == Alignment::Left,
        "format.alignCenter" => f.align == Alignment::Center,
        "format.alignRight" => f.align == Alignment::Right,
        "format.justify" => f.align == Alignment::Justify,
        "format.bullets" => f.bullets,
        "format.numbering" => f.numbering,
        _ => false,
    }
}

/// Whether command `id` can run now.
pub fn enabled(id: &str, f: &FormatState) -> bool {
    match id {
        "edit.undo" => f.can_undo,
        "edit.redo" => f.can_redo,
        "edit.copy" | "edit.cut" | "edit.paste" | "file.save" => true,
        _ if crate::doc_tools::enabled(id, f).is_some() => {
            crate::doc_tools::enabled(id, f).unwrap_or(false)
        }
        _ => {
            command(id).is_some()
                || WINDOW.contains(&id)
                || crate::slide_window::COMMANDS.contains(&id)
                || crate::catalog::is_menu(id)
                || crate::catalog::command(id, &f.section).is_some()
        }
    }
}
