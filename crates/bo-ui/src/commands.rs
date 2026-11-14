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

/// Keyboard shortcut shown in the tooltip of command `id`.
pub fn shortcut(id: &str) -> Option<&'static str> {
    Some(match id {
        "edit.undo" => "Ctrl+Z",
        "edit.redo" => "Ctrl+Y",
        "edit.cut" => "Ctrl+X",
        "edit.copy" => "Ctrl+C",
        "edit.paste" => "Ctrl+V",
        "file.save" => "Ctrl+S",
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
        _ => command(id).is_some(),
    }
}
