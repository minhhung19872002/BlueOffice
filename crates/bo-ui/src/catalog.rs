//! Commands of the ribbon tabs beyond Home: engine commands built from the JSON command bridge
//! (ADR-0008, the same shapes the React interface sends) and the choices of menu buttons.
use bo_edit::SectionState;
use bo_engine::edit::Command;
use serde_json::{json, Value};

/// An engine command from its JSON form (`{"cmd": …}`).
pub fn from_json(v: Value) -> Option<Command> {
    serde_json::from_value(v).ok()
}

/// The current section setup with `size` (twips) and / or `margins` (top, bottom, left, right
/// in twips) replaced; header, footer and columns are kept.
fn page_setup(
    sec: &SectionState,
    size: Option<(i32, i32)>,
    margins: Option<[i32; 4]>,
) -> Option<Command> {
    let (width, height) = size.unwrap_or((sec.width, sec.height));
    let [top, bottom, left, right] = margins.unwrap_or([
        sec.margin_top,
        sec.margin_bottom,
        sec.margin_left,
        sec.margin_right,
    ]);
    from_json(json!({
        "cmd": "setPageSetup",
        "setup": {
            "width": width, "height": height,
            "marginTop": top, "marginBottom": bottom,
            "marginLeft": left, "marginRight": right,
            "header": sec.header, "footer": sec.footer,
            "columns": sec.columns.max(1), "columnSpace": sec.column_space,
        },
        "scope": "document",
    }))
}

/// One entry of a menu button: its id, label key (or literal label) and command.
pub struct Entry {
    /// Choice id (`layout.margins.narrow`, …).
    pub id: &'static str,
    /// i18n key, or a literal label when it does not contain a dot.
    pub label: &'static str,
}

const fn e(id: &'static str, label: &'static str) -> Entry {
    Entry { id, label }
}

/// Entries of the menu button `id`.
pub fn menu(id: &str) -> Vec<Entry> {
    match id {
        "format.lineSpacing" => vec![
            e("format.lineSpacing1", "commands.format.lineSpacing1"),
            e("format.lineSpacing15", "commands.format.lineSpacing15"),
            e("format.lineSpacing2", "commands.format.lineSpacing2"),
        ],
        "format.changeCase" => vec![
            e("format.changeCase.sentence", "Sentence case"),
            e("format.changeCase.lower", "lowercase"),
            e("format.changeCase.upper", "UPPERCASE"),
            e("format.changeCase.title", "Capitalize Each Word"),
            e("format.changeCase.toggle", "tOGGLE cASE"),
        ],
        "format.shading" => vec![
            e("format.shading.none", "No color"),
            e("format.shading.FFFF00", "Yellow"),
            e("format.shading.D9D9D9", "Light gray"),
            e("format.shading.DDEBF7", "Light blue"),
            e("format.shading.E2EFDA", "Light green"),
            e("format.shading.FCE4D6", "Light orange"),
        ],
        "format.textEffects" => vec![
            e("format.textEffects.outline", "Outline"),
            e("format.textEffects.shadow", "Shadow"),
            e("format.textEffects.glow", "Glow"),
            e("format.textEffects.reflection", "Reflection"),
            e("format.textEffects.none", "No effects"),
        ],
        "format.multilevel" => vec![
            e("format.multilevel.outline", "1.  1.1.  1.1.1."),
            e("format.multilevel.mixed", "1)  a)  i)"),
            e("format.multilevel.bullets", "•  ◦  ▪"),
            e(
                "format.multilevel.headings",
                "1 Heading 1, 1.1 Heading 2, 1.1.1 Heading 3",
            ),
        ],
        "format.paraBorders" => vec![
            e("format.paraBorders.bottom", "Bottom border"),
            e("format.paraBorders.top", "Top border"),
            e("format.paraBorders.left", "Left border"),
            e("format.paraBorders.right", "Right border"),
            e("format.paraBorders.inside", "Inside border"),
            e("format.paraBorders.outside", "Outside borders"),
            e("format.paraBorders.all", "All borders"),
            e("format.paraBorders.none", "No border"),
        ],
        "format.sortParagraphs" => vec![
            e("format.sortParagraphs.asc", "Sort A to Z"),
            e("format.sortParagraphs.desc", "Sort Z to A"),
        ],
        "layout.margins" => vec![
            e("layout.margins.normal", "Normal — 2.54 cm"),
            e("layout.margins.narrow", "Narrow — 1.27 cm"),
            e("layout.margins.moderate", "Moderate — 2.54 / 1.91 cm"),
            e("layout.margins.wide", "Wide — 2.54 / 5.08 cm"),
            e("layout.margins.vn", "Vietnam (Decree 30) — 2 / 3 / 1.5 cm"),
        ],
        "layout.orientation" => vec![
            e("layout.orientation.portrait", "Portrait"),
            e("layout.orientation.landscape", "Landscape"),
        ],
        "layout.size" => vec![
            e("layout.size.a4", "A4 — 21 × 29.7 cm"),
            e("layout.size.letter", "Letter — 21.59 × 27.94 cm"),
            e("layout.size.legal", "Legal — 21.59 × 35.56 cm"),
            e("layout.size.a5", "A5 — 14.8 × 21 cm"),
            e("layout.size.a3", "A3 — 29.7 × 42 cm"),
        ],
        "layout.columns" => vec![
            e("layout.columns.1", "One"),
            e("layout.columns.2", "Two"),
            e("layout.columns.3", "Three"),
        ],
        "layout.breaks" => vec![
            e("insert.pageBreak", "Page"),
            e("layout.break.nextPage", "Next page (section)"),
            e("layout.break.continuous", "Continuous (section)"),
            e("layout.break.evenPage", "Even page (section)"),
            e("layout.break.oddPage", "Odd page (section)"),
        ],
        "insert.table" => vec![
            e("insert.table.2x2", "2 × 2"),
            e("insert.table.3x3", "3 × 3"),
            e("insert.table.4x4", "4 × 4"),
            e("insert.table.5x3", "5 columns × 3 rows"),
            e("insert.table.6x6", "6 × 6"),
        ],
        "insert.symbol" => vec![
            e("insert.symbol.©", "© Copyright"),
            e("insert.symbol.®", "® Registered"),
            e("insert.symbol.™", "™ Trade mark"),
            e("insert.symbol.€", "€ Euro"),
            e("insert.symbol.₫", "₫ Đồng"),
            e("insert.symbol.°", "° Degree"),
            e("insert.symbol.±", "± Plus-minus"),
            e("insert.symbol.×", "× Multiplication"),
            e("insert.symbol.÷", "÷ Division"),
            e("insert.symbol.≤", "≤ Less or equal"),
            e("insert.symbol.≥", "≥ Greater or equal"),
            e("insert.symbol.…", "… Ellipsis"),
            e("insert.symbol.—", "— Em dash"),
            e("insert.symbol.§", "§ Section"),
        ],
        "design.pageColor" => vec![
            e("design.pageColor.none", "No color"),
            e("design.pageColor.FFF8E1", "Cream"),
            e("design.pageColor.E8F0FD", "Light blue"),
            e("design.pageColor.E6F4EA", "Light green"),
            e("design.pageColor.F3F3F3", "Light gray"),
        ],
        "design.watermark" => vec![
            e("design.watermark.CONFIDENTIAL", "CONFIDENTIAL"),
            e("design.watermark.DRAFT", "DRAFT"),
            e("design.watermark.SAMPLE", "SAMPLE"),
            e("design.watermark.DO NOT COPY", "DO NOT COPY"),
            e("design.watermark.none", "Remove watermark"),
        ],
        "view.layout" => vec![
            e("view.layout.print", "Print Layout"),
            e("view.layout.web", "Web Layout"),
            e("view.layout.draft", "Draft"),
        ],
        _ => Vec::new(),
    }
}

/// Whether `id` is a menu button.
pub fn is_menu(id: &str) -> bool {
    !menu(id).is_empty()
}

/// The engine command of a tab button or menu entry, given the caret's section setup.
pub fn command(id: &str, sec: &SectionState) -> Option<Command> {
    let (w, h) = (sec.width, sec.height);
    let portrait = (w.min(h), w.max(h));
    let keep_orientation = |a: i32, b: i32| if w > h { (b, a) } else { (a, b) };
    // Centimetres to twips.
    let cm = |v: f32| (v * 1440.0 / 2.54).round() as i32;
    if let Some(sym) = id.strip_prefix("insert.symbol.") {
        return Some(Command::InsertText { text: sym.into() });
    }
    if let Some(dims) = id.strip_prefix("insert.table.") {
        let (c, r) = dims.split_once('x')?;
        return Some(Command::InsertTable {
            rows: r.parse().ok()?,
            cols: c.parse().ok()?,
        });
    }
    if let Some(effect) = id.strip_prefix("format.textEffects.") {
        // Text effects (w14 textOutline, shadow, glow, reflection; [MS-DOCX] 2.6).
        let patch = match effect {
            "outline" => json!({ "textOutline": { "color": "4472C4", "width": 0.75 } }),
            "shadow" => json!({ "softShadow": {
                "color": "000000", "blur": 4.0, "distance": 3.0, "angle": 45.0, "transparency": 60,
            } }),
            "glow" => json!({ "glow": { "color": "4472C4", "radius": 5.0, "transparency": 60 } }),
            "reflection" => json!({ "reflection": {
                "size": 50, "transparency": 50, "distance": 0.0,
            } }),
            "none" => json!({
                "textOutline": null, "softShadow": null, "glow": null, "reflection": null,
            }),
            _ => return None,
        };
        return from_json(json!({
            "cmd": "formatText",
            "format": { "kind": "advanced", "patch": patch },
        }));
    }
    if let Some(kind) = id.strip_prefix("format.multilevel.") {
        // ECMA-376 Part 1, §17.9.1 abstractNum / §17.9.6 lvl; headings link §17.9.23 pStyle.
        let level = |i: usize, format: &str, text: String, style: Option<String>| {
            json!({
                "format": format, "text": text,
                "indent": 360 * (i + 1), "hanging": 360,
                "style": style,
            })
        };
        let levels: Vec<Value> = match kind {
            "outline" => (0..9)
                .map(|i| {
                    let text = (1..=i + 1).fold(String::new(), |mut t, n| {
                        t.push_str(&format!("%{n}."));
                        t
                    });
                    level(i, "decimal", text, None)
                })
                .collect(),
            "mixed" => (0..9)
                .map(|i| {
                    let format = ["decimal", "lowerLetter", "lowerRoman"][i % 3];
                    level(i, format, format!("%{})", i + 1), None)
                })
                .collect(),
            "bullets" => (0..9)
                .map(|i| level(i, "bullet", ["•", "◦", "▪"][i % 3].into(), None))
                .collect(),
            "headings" => (0..3)
                .map(|i| {
                    let text = (1..=i + 1)
                        .map(|n| format!("%{n}"))
                        .collect::<Vec<_>>()
                        .join(".");
                    level(i, "decimal", text, Some(format!("Heading{}", i + 1)))
                })
                .collect(),
            _ => return None,
        };
        return from_json(json!({ "cmd": "defineList", "levels": levels, "apply": true }));
    }
    if let Some(preset) = id.strip_prefix("format.paraBorders.") {
        // ECMA-376 Part 1, §17.3.1.24 pBdr.
        return from_json(json!({
            "cmd": "formatParagraph",
            "format": { "kind": "borders", "preset": preset },
        }));
    }
    if let Some(order) = id.strip_prefix("format.sortParagraphs.") {
        return from_json(json!({ "cmd": "sortParagraphs", "descending": order == "desc" }));
    }
    if let Some(mode) = id.strip_prefix("format.changeCase.") {
        return from_json(json!({ "cmd": "changeCase", "mode": mode }));
    }
    if let Some(color) = id.strip_prefix("format.shading.") {
        // ECMA-376 Part 1, §17.3.1.31 shd (paragraph shading fill).
        let hex = (color != "none").then(|| color.to_owned());
        return Some(Command::FormatParagraph {
            format: bo_edit::ParaFormat::Shading { hex },
        });
    }
    if let Some(color) = id.strip_prefix("design.pageColor.") {
        let color = (color != "none").then(|| color.to_owned());
        return from_json(json!({ "cmd": "setPageColor", "color": color }));
    }
    if let Some(text) = id.strip_prefix("design.watermark.") {
        let watermark = (text != "none")
            .then(|| json!({ "text": text, "semitransparent": true, "diagonal": true }));
        return from_json(json!({ "cmd": "setWatermark", "watermark": watermark }));
    }
    if let Some(n) = id.strip_prefix("layout.columns.") {
        let count: u16 = n.parse().ok()?;
        return from_json(json!({
            "cmd": "setColumns",
            "columns": { "count": count, "space": 720, "separator": false, "widths": [] },
            "scope": "document",
        }));
    }
    if let Some(kind) = id.strip_prefix("layout.break.") {
        return from_json(json!({ "cmd": "insertSectionBreak", "kind": kind }));
    }
    if let Some(mode) = id.strip_prefix("view.layout.") {
        return from_json(json!({ "cmd": "setViewLayout", "mode": mode }));
    }
    let margins =
        |t: f32, b: f32, l: f32, r: f32| page_setup(sec, None, Some([cm(t), cm(b), cm(l), cm(r)]));
    let size = |a: i32, b: i32| page_setup(sec, Some(keep_orientation(a, b)), None);
    match id {
        "format.lineSpacing1" => Some(Command::FormatParagraph {
            format: bo_edit::ParaFormat::LineSpacing { value: 240 },
        }),
        "format.lineSpacing15" => Some(Command::FormatParagraph {
            format: bo_edit::ParaFormat::LineSpacing { value: 360 },
        }),
        "format.lineSpacing2" => Some(Command::FormatParagraph {
            format: bo_edit::ParaFormat::LineSpacing { value: 480 },
        }),
        "layout.margins.normal" => margins(2.54, 2.54, 2.54, 2.54),
        "layout.margins.narrow" => margins(1.27, 1.27, 1.27, 1.27),
        "layout.margins.moderate" => margins(2.54, 2.54, 1.91, 1.91),
        "layout.margins.wide" => margins(2.54, 2.54, 5.08, 5.08),
        "layout.margins.vn" => margins(2.0, 2.0, 3.0, 1.5),
        "layout.orientation.portrait" => page_setup(sec, Some(portrait), None),
        "layout.orientation.landscape" => page_setup(sec, Some((portrait.1, portrait.0)), None),
        "layout.size.a4" => size(cm(21.0), cm(29.7)),
        "layout.size.letter" => size(12240, 15840),
        "layout.size.legal" => size(12240, 20160),
        "layout.size.a5" => size(cm(14.8), cm(21.0)),
        "layout.size.a3" => size(cm(29.7), cm(42.0)),
        "insert.pageBreak" => Some(Command::InsertPageBreak),
        "insert.footnote" => from_json(json!({ "cmd": "insertNote", "endnote": false })),
        "insert.endnote" => from_json(json!({ "cmd": "insertNote", "endnote": true })),
        "insert.toc" | "references.toc" => Some(Command::InsertToc),
        "references.updateToc" | "fields.update" => from_json(json!({ "cmd": "updateFields" })),
        "insert.equation" => from_json(json!({ "cmd": "insertEquation", "text": "a^2+b^2=c^2" })),
        "insert.header" => {
            from_json(json!({ "cmd": "editHeaderFooter", "page": 0, "header": true }))
        }
        "insert.footer" => {
            from_json(json!({ "cmd": "editHeaderFooter", "page": 0, "header": false }))
        }
        "review.acceptAll" => from_json(json!({ "cmd": "acceptAllRevisions" })),
        "review.rejectAll" => from_json(json!({ "cmd": "rejectAllRevisions" })),
        "review.previous" => from_json(json!({ "cmd": "nextRevision", "forward": false })),
        "review.next" => from_json(json!({ "cmd": "nextRevision", "forward": true })),
        "review.previousComment" => from_json(json!({ "cmd": "nextComment", "forward": false })),
        "review.nextComment" => from_json(json!({ "cmd": "nextComment", "forward": true })),
        "format.clear" => from_json(json!({ "cmd": "clearFormatting" })),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_menu_entry_builds_an_engine_command() {
        let page = SectionState {
            width: 11906,
            height: 16838,
            margin_top: 1440,
            margin_bottom: 1440,
            margin_left: 1440,
            margin_right: 1440,
            header: 720,
            footer: 720,
            columns: 1,
            column_space: 720,
            ..SectionState::default()
        };
        for id in [
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
            "format.changeCase",
            "format.shading",
            "format.paraBorders",
            "format.sortParagraphs",
            "format.multilevel",
            "format.textEffects",
        ] {
            for entry in menu(id) {
                assert!(command(entry.id, &page).is_some(), "{}", entry.id);
            }
        }
        for id in [
            "insert.footnote",
            "insert.endnote",
            "insert.toc",
            "references.updateToc",
            "insert.header",
            "insert.footer",
            "review.acceptAll",
            "review.rejectAll",
            "review.next",
            "review.nextComment",
            "insert.equation",
            "format.clear",
        ] {
            assert!(command(id, &page).is_some(), "{id}");
        }
    }
}
